import React, { createContext, useContext, useReducer, type Dispatch } from 'react';
import type { RedactedSnapshot, RedactedApproval } from '../types/generated';

// 状态类型继承自 RedactedSnapshot
interface ApprovalState extends RedactedSnapshot {}

// Action 类型定义
type ApprovalAction =
  | { type: 'SNAPSHOT_UPDATED'; payload: RedactedSnapshot }
  | { type: 'OPTIMISTIC_DECIDE'; payload: { id: string; decision_kind: string } }
  | { type: 'DECIDE_FAILED'; payload: { id: string } };

// 初始空状态
const initialState: ApprovalState = { active: null, cached: [], revision: 0 };

// 审批状态 reducer
function approvalReducer(state: ApprovalState, action: ApprovalAction): ApprovalState {
  switch (action.type) {
    case 'SNAPSHOT_UPDATED':
      // 用服务端快照替换本地状态
      return { ...action.payload };
    case 'OPTIMISTIC_DECIDE': {
      // 乐观更新：立即反映决策结果
      if (!state.active || state.active.id !== action.payload.id) return state;
      return {
        ...state,
        active: {
          ...state.active,
          decision_kind: action.payload.decision_kind,
          state: 'decided',
        },
      };
    }
    case 'DECIDE_FAILED': {
      // 决策失败回滚：恢复 pending 状态
      if (!state.active || state.active.id !== action.payload.id) return state;
      return {
        ...state,
        active: { ...state.active, decision_kind: null, state: 'pending' },
      };
    }
    default:
      return state;
  }
}

// Context 值类型
interface ApprovalContextValue {
  state: ApprovalState;
  dispatch: Dispatch<ApprovalAction>;
}

const ApprovalContext = createContext<ApprovalContextValue | null>(null);

// Provider 组件
export function ApprovalProvider({ children }: { children: React.ReactNode }) {
  const [state, dispatch] = useReducer(approvalReducer, initialState);
  return (
    <ApprovalContext.Provider value={{ state, dispatch }}>
      {children}
    </ApprovalContext.Provider>
  );
}

// 消费 hook，必须在 ApprovalProvider 内使用
export function useApproval(): ApprovalContextValue {
  const ctx = useContext(ApprovalContext);
  if (!ctx) throw new Error('useApproval must be used within ApprovalProvider');
  return ctx;
}

// 导出类型供外部使用
export type { ApprovalAction, ApprovalState };
// 导出 RedactedApproval 供测试使用
export type { RedactedApproval };
