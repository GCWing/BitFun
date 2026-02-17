import { CheckIcon } from './CheckIcon';

interface CheckboxProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
}

export function Checkbox({ checked, onChange, label }: CheckboxProps) {
  return (
    <label className="checkbox-item" onClick={() => onChange(!checked)}>
      <div className={`checkbox-box ${checked ? 'checked' : ''}`}>
        <CheckIcon />
      </div>
      <span className="checkbox-label">{label}</span>
    </label>
  );
}
