import styles from './ActionBar.module.css';

// ActionBar 组件属性
interface ActionBarProps {
  onApprove: () => void;
  onDeny: () => void;
  disabled?: boolean;
}

// 审批操作栏：Approve / Deny 按钮
export function ActionBar({ onApprove, onDeny, disabled = false }: ActionBarProps) {
  return (
    <div className={styles.bar}>
      <button
        className={`${styles.btn} ${styles.approve}`}
        onClick={onApprove}
        disabled={disabled}
        type="button"
      >
        Approve
      </button>
      <button
        className={`${styles.btn} ${styles.deny}`}
        onClick={onDeny}
        disabled={disabled}
        type="button"
      >
        Deny
      </button>
    </div>
  );
}
