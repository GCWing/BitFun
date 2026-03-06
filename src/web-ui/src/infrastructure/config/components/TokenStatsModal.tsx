import React, { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { TrendingUp, Activity, MessageSquare } from 'lucide-react';
import { Modal, Button } from '@/component-library';
import { tokenUsageApi, ModelTokenStats, StatsTimeRange } from '@/infrastructure/api/tokenUsageApi';
import { createLogger } from '@/shared/utils/logger';
import './TokenStatsModal.scss';

const log = createLogger('TokenStatsModal');

interface TokenStatsModalProps {
  isOpen: boolean;
  onClose: () => void;
  modelId: string;
  modelName: string;
}

const TokenStatsModal: React.FC<TokenStatsModalProps> = ({
  isOpen,
  onClose,
  modelId,
  modelName
}) => {
  const { t } = useTranslation('settings/ai-model');
  const [stats, setStats] = useState<ModelTokenStats | null>(null);
  const [loading, setLoading] = useState(false);
  const [timeRange, setTimeRange] = useState<StatsTimeRange>('All');
  const [includeSubagent, setIncludeSubagent] = useState(false);

  const loadStats = useCallback(async () => {
    setLoading(true);
    try {
      const data = await tokenUsageApi.getModelStats(modelId, timeRange, includeSubagent);
      setStats(data);
    } catch (error) {
      log.error('Failed to load token stats', error);
    } finally {
      setLoading(false);
    }
  }, [modelId, timeRange, includeSubagent]);

  useEffect(() => {
    if (isOpen && modelId) {
      loadStats();
    }
  }, [isOpen, modelId, loadStats]);

  const formatNumber = (num: number) => {
    return num.toLocaleString();
  };

  const formatDate = (dateStr: string | null) => {
    if (!dateStr) return '-';
    return new Date(dateStr).toLocaleString();
  };

  const handleClearStats = async () => {
    if (!confirm(t('tokenStats.confirmClear'))) return;
    
    try {
      await tokenUsageApi.clearModelStats(modelId);
      await loadStats();
    } catch (error) {
      log.error('Failed to clear token stats', error);
    }
  };

  const timeRangeOptions: { value: StatsTimeRange; labelKey: string }[] = [
    { value: 'All', labelKey: 'tokenStats.rangeAll' },
    { value: 'Last30Days', labelKey: 'tokenStats.range30Days' },
    { value: 'Last7Days', labelKey: 'tokenStats.range7Days' },
  ];

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={`${modelName} - ${t('tokenStats.title')}`}
      size="medium"
    >
      <div className="token-stats-modal">
        <div className="token-stats-toolbar">
          <div className="token-stats-time-range">
            {timeRangeOptions.map(opt => (
              <button
                key={opt.value}
                className={`token-stats-range-btn ${timeRange === opt.value ? 'active' : ''}`}
                onClick={() => setTimeRange(opt.value)}
              >
                {t(opt.labelKey)}
              </button>
            ))}
          </div>
          <label className="token-stats-subagent-toggle">
            <input
              type="checkbox"
              checked={includeSubagent}
              onChange={(e) => setIncludeSubagent(e.target.checked)}
            />
            <span>{t('tokenStats.includeSubagent')}</span>
          </label>
        </div>

        {loading ? (
          <div className="token-stats-loading">
            <div className="spinner" />
            <p>{t('tokenStats.loading')}</p>
          </div>
        ) : stats ? (
          <>
            <div className="token-stats-grid">
              <div className="token-stat-card">
                <div className="token-stat-icon">
                  <TrendingUp size={20} />
                </div>
                <div className="token-stat-content">
                  <div className="token-stat-label">{t('tokenStats.totalTokens')}</div>
                  <div className="token-stat-value">{formatNumber(stats.total_tokens)}</div>
                </div>
              </div>

              <div className="token-stat-card">
                <div className="token-stat-icon">
                  <Activity size={20} />
                </div>
                <div className="token-stat-content">
                  <div className="token-stat-label">{t('tokenStats.inputTokens')}</div>
                  <div className="token-stat-value">{formatNumber(stats.total_input)}</div>
                </div>
              </div>

              <div className="token-stat-card">
                <div className="token-stat-icon">
                  <Activity size={20} />
                </div>
                <div className="token-stat-content">
                  <div className="token-stat-label">{t('tokenStats.outputTokens')}</div>
                  <div className="token-stat-value">{formatNumber(stats.total_output)}</div>
                </div>
              </div>

              <div className="token-stat-card">
                <div className="token-stat-icon">
                  <MessageSquare size={20} />
                </div>
                <div className="token-stat-content">
                  <div className="token-stat-label">{t('tokenStats.sessionCount')}</div>
                  <div className="token-stat-value">{stats.session_count}</div>
                </div>
              </div>
            </div>

            <div className="token-stats-details">
              <div className="token-stats-row">
                <span className="token-stats-label">{t('tokenStats.requestCount')}:</span>
                <span className="token-stats-value">{stats.request_count}</span>
              </div>
              <div className="token-stats-row">
                <span className="token-stats-label">{t('tokenStats.cachedTokens')}:</span>
                <span className="token-stats-value">{formatNumber(stats.total_cached)}</span>
              </div>
              <div className="token-stats-row">
                <span className="token-stats-label">{t('tokenStats.firstUsed')}:</span>
                <span className="token-stats-value">{formatDate(stats.first_used)}</span>
              </div>
              <div className="token-stats-row">
                <span className="token-stats-label">{t('tokenStats.lastUsed')}:</span>
                <span className="token-stats-value">{formatDate(stats.last_used)}</span>
              </div>
            </div>

            <div className="token-stats-actions">
              <Button variant="danger" onClick={handleClearStats}>
                {t('tokenStats.clearStats')}
              </Button>
            </div>
          </>
        ) : (
          <div className="token-stats-empty">
            <p>{t('tokenStats.noData')}</p>
          </div>
        )}
      </div>
    </Modal>
  );
};

export default TokenStatsModal;
