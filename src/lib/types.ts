// Rust（purpose-core）の型と対応させる。

export type Severity = "error" | "warning" | "info";

export interface Diagnostic {
  severity: Severity;
  code: string;
  message: string;
  file: string | null;
  field: string | null;
}

export type Status = "active" | "archived";

export interface PurposeFields {
  id: string;
  parent: string | null;
  name: string;
  purpose: string;
  rationale: string | null;
  principles: string[] | null;
  status: Status;
  valid_from: string | null;
  valid_until: string | null;
  review_at: string | null;
}

export type Effective = "active" | "archived" | "inherited_inactive" | "unknown";

export interface PurposeNode {
  id: string;
  file_name: string;
  path: string;
  hash: string;
  has_comments: boolean;
  fields: PurposeFields | null;
  raw_name: string | null;
  raw_parent: string | null;
  unknown_fields: string[];
  resolved: boolean;
  ancestors: string[];
  children: string[];
  effective: Effective;
  inactive_because: string | null;
  diagnostics: Diagnostic[];
}

export interface Snapshot {
  root: string;
  name: string;
  nodes: PurposeNode[];
  diagnostics: Diagnostic[];
}

export interface Settings {
  workspaces: string[];
  last: string | null;
}

export interface SaveRequest {
  fields: PurposeFields;
  expected_hash: string | null;
  allow_comment_loss: boolean;
}

export interface SaveOutcome {
  id: string;
  hash: string;
}

export type SaveError =
  | { kind: "validation"; diagnostics: Diagnostic[] }
  | { kind: "conflict"; current_hash: string | null }
  | { kind: "comments_would_be_lost" }
  | { kind: "already_exists" }
  | { kind: "not_editable"; diagnostics: Diagnostic[] }
  | { kind: "has_children"; children: string[] }
  | { kind: "workspace"; message: string }
  | { kind: "io"; message: string };

export type LoadFailure =
  | { kind: "not_found"; message: string }
  | { kind: "not_a_workspace"; message: string }
  | { kind: "io"; message: string };
