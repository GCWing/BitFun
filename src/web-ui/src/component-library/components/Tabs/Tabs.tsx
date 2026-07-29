import React, { useState, createContext, useContext, useMemo, useId } from 'react';
import './Tabs.scss';

export interface TabItem {
  key: string;
  label: React.ReactNode;
  icon?: React.ReactNode;
  disabled?: boolean;
  closable?: boolean;
  closeAriaLabel?: string;
}

export interface TabsProps {
  activeKey?: string;
  defaultActiveKey?: string;
  onChange?: (key: string) => void;
  onTabClose?: (key: string) => void;
  type?: 'line' | 'card' | 'pill';
  size?: 'small' | 'medium' | 'large';
  stretch?: boolean;
  children?: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
}

export interface TabPaneProps {
  tabKey: string;
  label: React.ReactNode;
  icon?: React.ReactNode;
  disabled?: boolean;
  closable?: boolean;
  closeAriaLabel?: string;
  children?: React.ReactNode;
  className?: string;
}

interface TabsContextValue {
  activeKey: string;
  onChange: (key: string) => void;
}

const TabsContext = createContext<TabsContextValue | undefined>(undefined);

export const TabPane: React.FC<TabPaneProps> = ({ children, className = '' }) => {
  const context = useContext(TabsContext);
  if (!context) return null;

  return <div className={`bitfun-tab-pane ${className}`}>{children}</div>;
};

TabPane.displayName = 'TabPane';

export const Tabs: React.FC<TabsProps> = ({
  activeKey: controlledActiveKey,
  defaultActiveKey,
  onChange,
  onTabClose,
  type = 'line',
  size = 'medium',
  stretch = false,
  children,
  className = '',
  style,
}) => {
  const generatedId = useId();
  const [internalActiveKey, setInternalActiveKey] = useState<string>(
    defaultActiveKey || ''
  );

  const activeKey = controlledActiveKey !== undefined ? controlledActiveKey : internalActiveKey;

  const { tabs, panes } = useMemo(() => {
    const nextTabs: TabItem[] = [];
    const nextPanes: { [key: string]: React.ReactNode } = {};

    React.Children.forEach(children, (child) => {
      if (React.isValidElement<TabPaneProps>(child) && child.type === TabPane) {
        const { tabKey, label, icon, disabled, closable, closeAriaLabel } = child.props;
        nextTabs.push({ key: tabKey, label, icon, disabled, closable, closeAriaLabel });
        nextPanes[tabKey] = child;
      }
    });

    return { tabs: nextTabs, panes: nextPanes };
  }, [children]);

  React.useEffect(() => {
    if (!activeKey && tabs.length > 0) {
      const firstKey = tabs[0].key;
      if (controlledActiveKey === undefined) {
        setInternalActiveKey(firstKey);
      }
    }
  }, [activeKey, controlledActiveKey, tabs]);

  const handleTabClick = (key: string, disabled?: boolean) => {
    if (disabled) return;
    
    if (controlledActiveKey === undefined) {
      setInternalActiveKey(key);
    }
    onChange?.(key);
  };

  const handleTabClose = (e: React.MouseEvent, key: string) => {
    e.stopPropagation();
    onTabClose?.(key);
  };

  const getTabId = (key: string) => `${generatedId}-tab-${key}`;
  const getPanelId = (key: string) => `${generatedId}-panel-${key}`;

  const handleTabKeyDown = (event: React.KeyboardEvent, currentIndex: number) => {
    const enabledTabs = tabs
      .map((tab, index) => ({ tab, index }))
      .filter(({ tab }) => !tab.disabled);
    const enabledIndex = enabledTabs.findIndex(({ index }) => index === currentIndex);
    let target: (typeof enabledTabs)[number] | undefined;

    if (event.key === 'ArrowRight') {
      target = enabledTabs[(enabledIndex + 1) % enabledTabs.length];
    } else if (event.key === 'ArrowLeft') {
      target = enabledTabs[(enabledIndex - 1 + enabledTabs.length) % enabledTabs.length];
    } else if (event.key === 'Home') {
      target = enabledTabs[0];
    } else if (event.key === 'End') {
      target = enabledTabs[enabledTabs.length - 1];
    } else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      handleTabClick(tabs[currentIndex].key, tabs[currentIndex].disabled);
      return;
    } else {
      return;
    }

    if (!target) return;
    event.preventDefault();
    handleTabClick(target.tab.key);
    document.getElementById(getTabId(target.tab.key))?.focus();
  };

  const containerClass = [
    'bitfun-tabs',
    `bitfun-tabs--${type}`,
    `bitfun-tabs--${size}`,
    stretch && 'bitfun-tabs--stretch',
    className
  ].filter(Boolean).join(' ');

  const contextValue: TabsContextValue = {
    activeKey,
    onChange: handleTabClick,
  };

  return (
    <TabsContext.Provider value={contextValue}>
      <div className={containerClass} style={style}>
        <div className="bitfun-tabs__nav">
          <div className="bitfun-tabs__nav-list" role="tablist">
            {tabs.map((tab, index) => (
              <div
                key={tab.key}
                className={[
                  'bitfun-tabs__tab',
                  activeKey === tab.key && 'bitfun-tabs__tab--active',
                  tab.disabled && 'bitfun-tabs__tab--disabled',
                ].filter(Boolean).join(' ')}
              >
                <button
                  id={getTabId(tab.key)}
                  className="bitfun-tabs__tab-button"
                  type="button"
                  onClick={() => handleTabClick(tab.key, tab.disabled)}
                  onKeyDown={(event) => handleTabKeyDown(event, index)}
                  role="tab"
                  tabIndex={activeKey === tab.key && !tab.disabled ? 0 : -1}
                  aria-selected={activeKey === tab.key}
                  aria-disabled={tab.disabled || undefined}
                  aria-controls={getPanelId(tab.key)}
                  disabled={tab.disabled}
                >
                  {tab.icon && <span className="bitfun-tabs__tab-icon">{tab.icon}</span>}
                  <span className="bitfun-tabs__tab-label">{tab.label}</span>
                </button>
                {tab.closable && (
                  <button
                    className="bitfun-tabs__tab-close"
                    type="button"
                    onClick={(e) => handleTabClose(e, tab.key)}
                    aria-label={tab.closeAriaLabel}
                  >
                    ×
                  </button>
                )}
              </div>
            ))}
          </div>
          {type === 'line' && <div className="bitfun-tabs__ink-bar" />}
        </div>
        <div
          id={getPanelId(activeKey)}
          className="bitfun-tabs__content"
          role="tabpanel"
          aria-labelledby={getTabId(activeKey)}
          tabIndex={0}
        >
          {panes[activeKey]}
        </div>
      </div>
    </TabsContext.Provider>
  );
};

Tabs.displayName = 'Tabs';
