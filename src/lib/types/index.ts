export interface GeneralConfig {
  language: string;
  theme: string;
  default_export_dir: string | null;
  recent_files_max: number;
}

export type S3AuthMode = "none" | "static" | "env";

export interface S3Config {
  auth_mode: S3AuthMode;
  endpoint: string;
  region: string;
  bucket: string;
  access_key: string;
  secret_key: string;
  session_token: string | null;
  force_path_style: boolean;
  root_prefix: string | null;
  max_versions: number | null;
  version_ttl_days: number | null;
}

export interface AppConfig {
  general: GeneralConfig;
  s3: S3Config | null;
  ocr: OcrConfig | null;
}

export interface OcrConfig {
  modelDir: string;
  detModel: string;
  recModel: string;
  keysFile: string;
  language: string;
  gpuEnabled: boolean;
}

export interface OcrModelSet {
  detPath: string;
  recPath: string;
  keysPath: string;
  language: string;
  displayName: string;
}

export interface OcrSuggestedModel {
  name: string;
  language: string;
  description: string;
  detUrl: string;
  recUrl: string;
  keysUrl: string;
  totalSize: string;
}

export interface PdfInfo {
  isEncrypted: boolean;
  pages: number;
  fileSize: number;
  title: string | null;
  author: string | null;
  subject: string | null;
  keywords: string | null;
  creator: string | null;
  producer: string | null;
  creationDate: string | null;
  modDate: string | null;
  pdfVersion: string | null;
  pageSize: string | null;
}
