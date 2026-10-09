import { invoke } from "@tauri-apps/api/core";

export type Status = "pending" | "approved" | "rejected" | "applied";

export interface Video {
  id: string;
  title: string;
  description: string;
  category_id?: string | null;
  published_at?: string | null;
  tags?: string[] | null;
  kind: string;
}

export interface Proposal {
  video_id: string;
  original_title: string;
  proposed_title: string;
  proposed_description: string;
  status: Status;
}

export interface StatusDto {
  logged_in: boolean;
  has_credentials: boolean;
  model: string;
  ollama_url: string;
  prompt: string;
  videos: number;
  pending: number;
  approved: number;
  rejected: number;
  applied: number;
}

export interface ApplyResult {
  applied: number;
  failed: number;
}

export const api = {
  getStatus: () => invoke<StatusDto>("get_status"),
  listVideos: () => invoke<Video[]>("list_videos"),
  listProposals: () => invoke<Proposal[]>("list_proposals"),
  pull: (limit: number | null) => invoke<number>("pull", { limit }),
  generate: (model: string | null, redo: boolean) =>
    invoke<Proposal[]>("generate", { model, redo }),
  editProposal: (videoId: string, title: string, description?: string) =>
    invoke<void>("edit_proposal", { videoId, title, description: description ?? null }),
  setStatus: (videoId: string, status: Status) =>
    invoke<void>("set_status", { videoId, status }),
  apply: () => invoke<ApplyResult>("apply"),
  saveCredentials: (clientId: string, clientSecret: string, json: string) =>
    invoke<void>("save_credentials", {
      clientId: clientId || null,
      clientSecret: clientSecret || null,
      json: json || null,
    }),
  login: () => invoke<void>("login"),
  setModel: (model: string) => invoke<void>("set_model", { model }),
  setOllamaUrl: (url: string) => invoke<void>("set_ollama_url", { url }),
  savePrompt: (text: string) => invoke<void>("save_prompt", { text }),
};
