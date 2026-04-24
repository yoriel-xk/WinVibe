// IPC 返回的脱敏审批数据
export interface RedactedApproval {
  id: string;
  session_hash: string;
  tool_name: string;
  fingerprint: string;
  fingerprint_version: number;
  state: "pending" | "decided";
  decision_kind: string | null; // "Approved" | "Denied" | "TimedOut" | "Cancelled"
  feedback: string | null;
  created_wall: string; // RFC 3339
}

// 快照：当前活跃审批 + 历史缓存
export interface RedactedSnapshot {
  active: RedactedApproval | null;
  cached: RedactedApproval[];
  revision: number;
}

// HUD 决策请求体
export interface HudDecision {
  kind: "Approved" | "Denied";
  feedback?: string;
}

// IPC 错误结构
export interface IpcError {
  code: string;
  message: string;
}

// IPC 事件载荷
export interface IpcEvent {
  approval_id: string;
  revision: number;
}
