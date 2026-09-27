/**
 * Biến phiên bản ứng dụng trung tâm (Single Source of Truth cho toàn bộ UI).
 * Được nạp tự động từ package.json qua Vite compile-time definition (__APP_VERSION__).
 * Nếu không có Vite define, sẽ sử dụng giá trị fallback bên dưới.
 */
export const APP_VERSION = typeof __APP_VERSION__ !== 'undefined' ? __APP_VERSION__ : '1.1.0';

/** Nhãn hiển thị phiên bản (Ví dụ: "v1.1.0") */
export const APP_VERSION_TAG = `v${APP_VERSION}`;

/** Nhãn hiển thị Engine trên thanh quản lý lệnh & cài đặt (Ví dụ: "v1.1.0 Engine") */
export const APP_ENGINE_TAG = `${APP_VERSION_TAG} Engine`;

/** Nhãn hiển thị Sequencer Orchestrator trên quản lý nhóm (Ví dụ: "v1.1.0 Orchestrator") */
export const APP_ORCHESTRATOR_TAG = `${APP_VERSION_TAG} Orchestrator`;
