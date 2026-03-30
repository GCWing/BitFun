import React, { useState, createContext, useContext, useMemo } from 'react';
import './Tabs.scss';

export interface TabItem {
  key: string;
  label: React.ReactNode;
  icon?: React.ReactNode;
  disabled?: boolean;
  closable?: boolean;
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
  const [internalActiveKey, setInternalActiveKey] = useState<string>(
    defaultActiveKey || ''
  );

  const activeKey = controlledActiveKey !== undefined ? controlledActiveKey : internalActiveKey;

  const { tabs, panes } = useMemo(() => {
    const nextTabs: TabItem[] = [];
    const nextPanes: { [key: string]: React.ReactNode } = {};

    React.Children.forEach(children, (child) => {
      if (React.isValidElement<TabPaneProps>(child) && child.type === TabPane) {
        const { tabKey, label, icon, disabled, closable } = child.props;
        nextTabs.push({ key: tabKey, label, icon, disabled, closable });
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
          <div className="bitfun-tabs__nav-list">
            {tabs.map((tab) => (
              <div
                key={tab.key}
                className={[
                  'bitfun-tabs__tab',
                  activeKey === tab.key && 'bitfun-tabs__tab--active',
                  tab.disabled && 'bitfun-tabs__tab--disabled',
                ].filter(Boolean).join(' ')}
                onClick={() => handleTabClick(tab.key, tab.disabled)}
              >
                {tab.icon && <span className="bitfun-tabs__tab-icon">{tab.icon}</span>}
                <span className="bitfun-tabs__tab-label">{tab.label}</span>
                {tab.closable && (
                  <span
                    className="bitfun-tabs__tab-close"
                    onClick={(e) => handleTabClose(e, tab.key)}
                  >
                    ×
                  </span>
                )}
              </div>
            ))}
          </div>
          {type === 'line' && <div className="bitfun-tabs__ink-bar" />}
        </div>
        <div className="bitfun-tabs__content">
          {panes[activeKey]}
        </div>
      </div>
    </TabsContext.Provider>
  );
};

Tabs.displayName = 'Tabs';
