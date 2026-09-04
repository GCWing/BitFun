interface ProgressBarProps {
  percent: number;
  completed?: boolean;
}

export function ProgressBar({ percent, completed = false }: ProgressBarProps) {
  return (
    <div className="progress-bar-container">
      <div
        className="progress-bar-fill"
        style={{
          width: `${Math.min(100, Math.max(0, percent))}%`,
          background: completed ? 'var(--openbitfun-color-status-success-content)' : 'var(--openbitfun-color-accent-default)',
        }}
      />
    </div>
  );
}
