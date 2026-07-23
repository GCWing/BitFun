import React, { useEffect, forwardRef } from 'react'
import './EditArea.scss'

interface EditAreaProps {
  value: string
  onChange: (value: string) => void
  onFocus?: () => void
  onBlur?: () => void
  placeholder?: string
  readonly?: boolean
  autofocus?: boolean
}

export const EditArea = forwardRef<HTMLTextAreaElement, EditAreaProps>(
  ({ value, onChange, onFocus, onBlur, placeholder, readonly, autofocus }, ref) => {
    useEffect(() => {
      if (autofocus && ref && 'current' in ref && ref.current) {
        ref.current.focus()
      }
    }, [autofocus, ref])

    const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      onChange(e.target.value)
    }

    const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Tab') {
        e.preventDefault()
        const textarea = e.currentTarget
        const start = textarea.selectionStart
        const end = textarea.selectionEnd
        const newValue = value.substring(0, start) + '  ' + value.substring(end)
        onChange(newValue)
        
        setTimeout(() => {
          textarea.selectionStart = textarea.selectionEnd = start + 2
        }, 0)
      }
    }

    return (
      <textarea
        ref={ref}
        className="m-editor-textarea"
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        onFocus={onFocus}
        onBlur={onBlur}
        placeholder={placeholder}
        readOnly={readonly}
        spellCheck={false}
      />
    )
  }
)

EditArea.displayName = 'EditArea'

