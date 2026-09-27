import { ref, computed } from 'vue';
import type {
  CommandTemplate,
  TemplateParam,
  TemplatePreset,
  TemplatePreviewPayload,
  TemplateRunPayload,
} from '@/types/models';
import { ipcClient, isTauriRuntime } from '@/ipc/client';

// Sample mock data for development mode
const initialTemplates: CommandTemplate[] = [
  {
    id: 1,
    name: 'FFmpeg Video Converter',
    description: 'Transcode video files with custom CRF quality and optional decryption key',
    template_string: 'ffmpeg -i {{input}} -crf {{crf}} -key {{secret_key}} {{output}}',
    is_shell: true,
    last_run_at: '10 mins ago',
    preset_count: 2,
    params: [
      {
        name: 'input',
        label: 'File Video Input',
        kind: 'path',
        default_value: '/home/user/Videos/input_sample.mp4',
        required: true,
        is_secret: false,
        param_order: 1,
      },
      {
        name: 'crf',
        label: 'CRF Compression Level',
        kind: 'number',
        default_value: '23',
        required: false,
        is_secret: false,
        param_order: 2,
      },
      {
        name: 'secret_key',
        label: 'Decryption Password',
        kind: 'string',
        default_value: '',
        required: false,
        is_secret: true,
        param_order: 3,
      },
      {
        name: 'output',
        label: 'Output File Path',
        kind: 'string',
        default_value: '/home/user/Videos/output_compressed.mp4',
        required: true,
        is_secret: false,
        param_order: 4,
      },
    ],
  },
  {
    id: 2,
    name: 'Curl API Request',
    description: 'Send HTTP requests with bearer token authorization',
    template_string: 'curl -X {{method}} {{url}} -H {{auth_token}}',
    is_shell: false,
    last_run_at: 'Yesterday',
    preset_count: 2,
    params: [
      {
        name: 'method',
        label: 'HTTP Method',
        kind: 'enum',
        default_value: 'GET',
        options: ['GET', 'POST', 'PUT', 'DELETE', 'PATCH'],
        required: true,
        is_secret: false,
        param_order: 1,
      },
      {
        name: 'url',
        label: 'Endpoint URL',
        kind: 'string',
        default_value: 'https://api.example.com/v1/health',
        required: true,
        is_secret: false,
        param_order: 2,
      },
      {
        name: 'auth_token',
        label: 'Bearer Authorization Token',
        kind: 'string',
        default_value: '',
        required: false,
        is_secret: true,
        param_order: 3,
      },
    ],
  },
  {
    id: 3,
    name: 'Docker Container Runner',
    description: 'Spin up isolated container instances with exposed port bindings',
    template_string: 'docker run -d -p {{host_port}}:{{port}} {{image}}',
    is_shell: true,
    last_run_at: '3 days ago',
    preset_count: 1,
    params: [
      {
        name: 'host_port',
        label: 'Host Port Binding',
        kind: 'number',
        default_value: '8080',
        required: true,
        is_secret: false,
        param_order: 1,
      },
      {
        name: 'port',
        label: 'Container Port',
        kind: 'number',
        default_value: '80',
        required: true,
        is_secret: false,
        param_order: 2,
      },
      {
        name: 'image',
        label: 'Container Image Tag',
        kind: 'string',
        default_value: 'nginx:alpine',
        required: true,
        is_secret: false,
        param_order: 3,
      },
    ],
  },
  {
    id: 4,
    name: 'PostgreSQL Database Dump',
    description: 'Export database schema and records using pg_dump utility',
    template_string: 'pg_dump -h {{host}} -U {{user}} {{dbname}}',
    is_shell: false,
    last_run_at: 'Never',
    preset_count: 1,
    params: [
      {
        name: 'host',
        label: 'PostgreSQL Host',
        kind: 'string',
        default_value: 'localhost',
        required: true,
        is_secret: false,
        param_order: 1,
      },
      {
        name: 'user',
        label: 'DB Username',
        kind: 'string',
        default_value: 'postgres',
        required: true,
        is_secret: false,
        param_order: 2,
      },
      {
        name: 'password',
        label: 'DB Password',
        kind: 'string',
        default_value: '',
        required: false,
        is_secret: true,
        param_order: 3,
      },
      {
        name: 'dbname',
        label: 'Database Name',
        kind: 'string',
        default_value: 'app_production',
        required: true,
        is_secret: false,
        param_order: 4,
      },
    ],
  },
];

const initialPresets: TemplatePreset[] = [
  {
    id: 101,
    template_id: 1,
    name: 'H.264 1080p Standard',
    values: {
      input: '/home/user/Videos/input_sample.mp4',
      crf: '23',
      output: '/home/user/Videos/output_compressed.mp4',
    },
    last_used_at: '10 mins ago',
  },
  {
    id: 102,
    template_id: 1,
    name: 'H.265 High Quality (CRF 18)',
    values: {
      input: '/home/user/Videos/raw_video.mov',
      crf: '18',
      output: '/home/user/Videos/output_h265.mp4',
    },
    last_used_at: '2 days ago',
  },
  {
    id: 201,
    template_id: 2,
    name: 'Health Check GET',
    values: {
      method: 'GET',
      url: 'https://api.example.com/v1/health',
    },
    last_used_at: 'Yesterday',
  },
];

// Browser preview shares these definitions with the Templates screen so the
// suggestion engine does not silently drop the template source without Tauri.
export const mockTemplates = initialTemplates;

const templates = ref<CommandTemplate[]>(initialTemplates);
const presets = ref<TemplatePreset[]>(initialPresets);
const loading = ref(false);
const error = ref<string | null>(null);

export function useTemplates() {
  const shellTemplatesCount = computed(() =>
    templates.value.filter(t => t.is_shell).length
  );

  const argvTemplatesCount = computed(() =>
    templates.value.filter(t => !t.is_shell).length
  );

  const totalPresetsCount = computed(() => presets.value.length);

  const fetchTemplates = async () => {
    loading.value = true;
    error.value = null;
    try {
      if (isTauriRuntime()) {
        const loaded = await ipcClient.listTemplates();
        templates.value = loaded;
        presets.value = loaded.flatMap(template => template.presets || []);
      }
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : String(err);
    } finally {
      loading.value = false;
    }
  };

  const getTemplateById = (id: number) => {
    return templates.value.find(t => t.id === id);
  };

  const createTemplate = async (newTpl: Omit<CommandTemplate, 'id'>) => {
    if (isTauriRuntime()) {
      loading.value = true;
      error.value = null;
      try {
        await ipcClient.saveTemplate(newTpl);
        await fetchTemplates();
        return templates.value[templates.value.length - 1];
      } catch (err: unknown) {
        error.value = err instanceof Error ? err.message : String(err);
        return undefined;
      } finally {
        loading.value = false;
      }
    }

    const nextId = Math.max(0, ...templates.value.map(t => t.id)) + 1;
    const template: CommandTemplate = {
      ...newTpl,
      id: nextId,
      last_run_at: 'Chưa chạy',
      preset_count: 0,
    };
    templates.value.push(template);
    return template;
  };

  const updateTemplate = async (id: number, updated: Partial<CommandTemplate>) => {
    if (isTauriRuntime()) {
      const existing = getTemplateById(id);
      if (!existing) return;
      loading.value = true;
      error.value = null;
      try {
        await ipcClient.saveTemplate({
          ...existing,
          ...updated,
          id,
          params: updated.params ?? existing.params,
        });
        await fetchTemplates();
      } catch (err: unknown) {
        error.value = err instanceof Error ? err.message : String(err);
      } finally {
        loading.value = false;
      }
      return;
    }

    const index = templates.value.findIndex(t => t.id === id);
    if (index !== -1) {
      templates.value[index] = {
        ...templates.value[index],
        ...updated,
      };
    }
  };

  const deleteTemplate = async (id: number) => {
    if (isTauriRuntime()) {
      loading.value = true;
      error.value = null;
      try {
        await ipcClient.deleteTemplate(id);
        await fetchTemplates();
      } catch (err: unknown) {
        error.value = err instanceof Error ? err.message : String(err);
      } finally {
        loading.value = false;
      }
      return;
    }

    templates.value = templates.value.filter(t => t.id !== id);
    presets.value = presets.value.filter(p => p.template_id !== id);
  };

  const fetchPresetsForTemplate = (templateId: number) => {
    return presets.value.filter(p => p.template_id === templateId);
  };

  const savePreset = async (templateId: number, name: string, values: Record<string, string>) => {
    const tpl = getTemplateById(templateId);
    if (!tpl) return;

    if (isTauriRuntime()) {
      loading.value = true;
      error.value = null;
      try {
        const saved = await ipcClient.saveTemplatePreset(templateId, name, values);
        await fetchTemplates();
        return presets.value.find(p => p.id === saved.id) || saved;
      } catch (err: unknown) {
        error.value = err instanceof Error ? err.message : String(err);
        return undefined;
      } finally {
        loading.value = false;
      }
    }

    // Filter out secret parameters from preset storage
    const cleanValues: Record<string, string> = {};
    tpl.params.forEach(p => {
      if (!p.is_secret && values[p.name] !== undefined) {
        cleanValues[p.name] = values[p.name];
      }
    });

    const nextId = Math.max(0, ...presets.value.map(p => p.id)) + 1;
    const newPreset: TemplatePreset = {
      id: nextId,
      template_id: templateId,
      name,
      values: cleanValues,
      last_used_at: 'Just now',
    };
    presets.value.push(newPreset);
    tpl.preset_count = (tpl.preset_count || 0) + 1;
    return newPreset;
  };

  const deletePreset = async (presetId: number) => {
    if (isTauriRuntime()) {
      loading.value = true;
      error.value = null;
      try {
        await ipcClient.deleteTemplatePreset(presetId);
        await fetchTemplates();
      } catch (err: unknown) {
        error.value = err instanceof Error ? err.message : String(err);
      } finally {
        loading.value = false;
      }
      return;
    }

    const p = presets.value.find(item => item.id === presetId);
    if (p) {
      const tpl = getTemplateById(p.template_id);
      if (tpl && tpl.preset_count) tpl.preset_count--;
      presets.value = presets.value.filter(item => item.id !== presetId);
    }
  };

  /**
   * Helper to parse {{param_name}} placeholders from a template string
   */
  const extractPlaceholders = (templateString: string): string[] => {
    const regex = /\{\{\s*([a-zA-Z0-9_]+)\s*\}\}/g;
    const matches = new Set<string>();
    let match;
    while ((match = regex.exec(templateString)) !== null) {
      if (match[1]) {
        matches.add(match[1]);
      }
    }
    return Array.from(matches);
  };

  /**
   * Render preview of template execution string safely (quoting for shell / masked secrets)
   */
  const previewTemplate = (
    template: CommandTemplate,
    paramValues: Record<string, string>
  ): TemplatePreviewPayload => {
    let rawResult = template.template_string;
    let maskedResult = template.template_string;
    const errors: Record<string, string> = {};

    template.params.forEach(param => {
      const val = paramValues[param.name] ?? param.default_value ?? '';
      if (param.required && !val.trim()) {
        errors[param.name] = `Tham số ${param.label || param.name} là bắt buộc.`;
      }

      // Safe quoting for shell execution mode if string contains spaces
      const formattedVal = template.is_shell && val.includes(' ') ? `"${val}"` : val;
      const placeholderRegex = new RegExp(`\\{\\{\\s*${param.name}\\s*\\}\\}`, 'g');

      rawResult = rawResult.replace(placeholderRegex, formattedVal || `{{${param.name}}}`);

      if (param.is_secret) {
        maskedResult = maskedResult.replace(placeholderRegex, '"******"');
      } else {
        maskedResult = maskedResult.replace(placeholderRegex, formattedVal || `{{${param.name}}}`);
      }
    });

    return {
      rendered: rawResult,
      masked_rendered: maskedResult,
      is_shell: template.is_shell,
      validation_errors: Object.keys(errors).length > 0 ? errors : undefined,
    };
  };

  return {
    templates,
    presets,
    loading,
    error,
    shellTemplatesCount,
    argvTemplatesCount,
    totalPresetsCount,
    fetchTemplates,
    getTemplateById,
    createTemplate,
    updateTemplate,
    deleteTemplate,
    fetchPresetsForTemplate,
    savePreset,
    deletePreset,
    extractPlaceholders,
    previewTemplate,
  };
}
