export interface SessionView {
  id: string; name: string; group: string; host: string; port: number; username: string;
  savePassword: boolean; hasPassword: boolean; hasPassphrase: boolean;
  keyFilePath: string | null; keyFileMissing: boolean;
  createdAt: string; lastConnectedAt: string | null;
  displayName: string; subtitle: string; effectiveGroup: string;
}
export interface SessionDraft {
  id?: string | null; name: string; group: string; host: string; port: number; username: string;
  savePassword: boolean; password?: string | null; clearPassword?: boolean;
  keyFilePath?: string | null; passphrase?: string | null;
}
export interface AppSettings {
  FontFamily: string; FontSize: number; Language: string; Theme: string; CopyOnSelect: boolean;
  RightClickAction: string; ConfirmMultilinePaste: boolean; Scrollback: number; CursorBlink: boolean;
  KeepAliveSeconds: number; ShowHiddenFiles: boolean; CustomEditorPath: string; CollapsedGroups: string[];
  LastExportFolder: string; LastImportFolder: string; MaxParallelConnects: number;
  WindowWidth: number; WindowHeight: number; LeftColumnWidth: number;
  [k: string]: unknown;
}
export type ConnectionStatus = 'disconnected' | 'connecting' | 'connected' | 'reconnecting';
export interface TabStatusEvent { tabId: string; sessionId: string; status: ConnectionStatus; message: string; }
export interface DialogRequest {
  requestId: string; tabId: string; kind: 'hostKey' | 'password'; title: string; host: string; port: number;
  username: string; algorithm: string; fingerprint: string; changed: boolean; wasRejected: boolean;
}
export interface MonitorInfo {
  osGroup: string; hostname: string; cpuPercent: number; ramPercent: number; ramText: string; uploadText: string;
  downloadText: string; uptimeText: string; username: string; diskText: string; osPretty: string; kernel: string; arch: string; dfOutput: string;
}
export interface SftpItem {
  name: string; fullName: string; isDirectory: boolean; isParentDirectory: boolean; isSymbolicLink: boolean;
  size: number; lastModified: number; permissions: string; mode: number; userId: number; groupId: number; owner: string; group: string;
}
export interface SftpListing { path: string; home: string; items: SftpItem[]; }
export interface TransferProgress { tabId: string; transferId: string; name: string; done: number; total: number; speedBps: number; finished: boolean; error: string | null; }
export interface ImportPreviewItem { id: string; name: string; subtitle: string; group: string; status: 'new' | 'duplicate' | 'invalid'; existingName: string | null; hasSecrets: boolean; keyFileName: string | null; }
export interface ImportFileInfo { path: string; fileName: string; format: string; count: number; protected: boolean; items: ImportPreviewItem[]; }
export interface ImportResult { totalInFile: number; importedCount: number; skippedCount: number; overwrittenCount: number; addedCopyCount: number; corruptSecretsCount: number; messages: string[]; }
export interface ExportOutcome { exported: number; path: string; messages: string[]; }
export interface AppInfo { version: string; dataDir: string; localDir: string; logsDir: string; backupsDir: string; platform: string; }

export interface SyncBackendConfig { backendType: 'Folder' | 'Sftp'; folderPath: string; sftpSessionId: string | null; sftpRemotePath: string; }
export interface SyncStatus {
  enabled: boolean; state: 'never' | 'idle' | 'running' | 'offline' | 'needPassword' | 'error';
  lastSyncAt: string | null; lastError: string | null; backendLabel: string; backendType: string; folderPath: string;
  sftpSessionId: string | null; sftpRemotePath: string; intervalMinutes: number; includeKeyFiles: boolean; lastRevision: number; deviceName: string;
}
export interface MergeReport { addLocal: string[]; updateLocal: string[]; deleteLocal: string[]; toRemote: number; mergedDuplicates: string[]; }
export interface SyncPreview { remoteExisted: boolean; report: MergeReport; localCount: number; messages: string[]; }
export interface SyncOutcome { report: MergeReport; localChanged: boolean; remoteWritten: boolean; revision: number; remoteExisted: boolean; messages: string[]; }
export interface SyncTestResult { vaultExists: boolean; label: string; }
