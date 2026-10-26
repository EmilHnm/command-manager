export interface FolderParamConfig {
  path: string;
  deep: boolean;
}

const FOLDER_PREFIX = '__cm_folder__:';

export function parseFolderConfigFromOptions(options?: string[]): {
  isFolder: boolean;
  config: FolderParamConfig | null;
  files: string[];
} {
  if (!options || options.length === 0) {
    return { isFolder: false, config: null, files: [] };
  }
  const first = options[0];
  if (typeof first === 'string' && first.startsWith(FOLDER_PREFIX)) {
    try {
      const config = JSON.parse(first.slice(FOLDER_PREFIX.length)) as FolderParamConfig;
      return {
        isFolder: true,
        config,
        files: options.slice(1),
      };
    } catch {
      return { isFolder: false, config: null, files: options };
    }
  }
  return { isFolder: false, config: null, files: options };
}

export function buildOptionsWithFolderConfig(files: string[], config: FolderParamConfig): string[] {
  return [FOLDER_PREFIX + JSON.stringify(config), ...files];
}
