import { Component, type ReactNode, type ErrorInfo } from 'react';
import styles from './ErrorBoundary.module.css';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

// React ErrorBoundary：捕获子组件渲染异常，防止整个 HUD 白屏
export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, error: null };

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('[ErrorBoundary] 渲染异常:', error, info.componentStack);
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className={styles.container}>
          <h2 className={styles.title}>Something went wrong</h2>
          <pre className={styles.message}>{this.state.error?.message}</pre>
          <button className={styles.retry} onClick={this.handleRetry}>
            Retry
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
