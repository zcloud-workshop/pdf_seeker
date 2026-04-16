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
}
