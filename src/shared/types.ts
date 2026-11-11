export type HeaderTitle = 'project-directory' | 'repository-branch';
export type HeaderSubtitle = HeaderTitle | 'none';
export type Theme = 'system' | 'light' | 'dark';
export type ProjectSortOrder = 'project-directory-asc' | 'updated-at-desc' | 'updated-at-asc';

export type StatusSortOrder = 'submitted-at-desc' | 'submitted-at-asc';

export type StatusPoint = {
  text: string;
  done: boolean;
  link: string | null;
  submitted_at?: string | null;
  id?: string | null;
};
export type StatusEntry = {
  repository?: string | null;
  folder_name: string;
  session_id: string | null;
  branch: string;
  points: StatusPoint[];
  updated_at: string;
};
export type Snapshot = {
  path: string | null;
  document: { version: 1 | 2; entries: StatusEntry[] };
  revision: number;
  health: 'waiting' | 'current' | 'stale' | 'error';
  error: string | null;
  modified_at: string | null;
};

export type AgentInstallation = {
  id: string;
  name: string;
  skillInstalled: boolean;
  mcpRegistered: boolean;
  available: boolean;
  configPath: string;
  error: string | null;
};

export type SkillTarget = {
  id: string;
  name: string;
  path: string;
  installed: boolean;
  upToDate: boolean;
};

export type SkillInstallation = {
  installed: boolean;
  mcpRegistered: boolean;
  path: string;
  targets: SkillTarget[];
  additionalTargets: SkillTarget[];
  agents: AgentInstallation[];
  additionalAgents: AgentInstallation[];
};

export type Settings = {
  headerTitle: HeaderTitle;
  headerSubtitle: HeaderSubtitle;
  theme: Theme;
  primaryColor: string;
  textScale: number;
  pinned: boolean;
  projectSortOrder: ProjectSortOrder;
  statusSortOrder: StatusSortOrder;
  autoClearAfterMinutes: number | null;
  maxStatusItemsPerProject: number | null;
  undoLimit: number;
};
