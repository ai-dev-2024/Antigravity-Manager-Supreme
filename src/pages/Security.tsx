import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Shield, Lock, FileText, Settings, Activity, RefreshCw } from 'lucide-react';
import { IpAccessLogs } from '../components/security/IpAccessLogs';
import { BlacklistManager } from '../components/security/BlacklistManager';
import { WhitelistManager } from '../components/security/WhitelistManager';
import { SecurityConfig } from '../components/security/SecurityConfig';
import { IpStatistics } from '../components/security/IpStatistics';

const Security: React.FC = () => {
    const { t } = useTranslation();
    const [activeTab, setActiveTab] = useState<'logs' | 'stats' | 'blacklist' | 'whitelist' | 'config'>('logs');
    const [refreshKey, setRefreshKey] = useState(0);

    const handleRefresh = () => {
        setRefreshKey(prev => prev + 1);
    };

    const renderContent = () => {
        switch (activeTab) {
            case 'logs':
                return <IpAccessLogs refreshKey={refreshKey} />;
            case 'stats':
                return <IpStatistics refreshKey={refreshKey} />;
            case 'blacklist':
                return <BlacklistManager refreshKey={refreshKey} />;
            case 'whitelist':
                return <WhitelistManager refreshKey={refreshKey} />;
            case 'config':
                return <SecurityConfig />;
            default:
                return <IpAccessLogs refreshKey={refreshKey} />;
        }
    };

    const tabs = [
        { id: 'logs', label: t('security.tab_logs'), icon: FileText },
        { id: 'stats', label: t('security.tab_stats'), icon: Activity },
        { id: 'blacklist', label: t('security.tab_blacklist'), icon: Shield },
        { id: 'whitelist', label: t('security.tab_whitelist'), icon: Lock },
        { id: 'config', label: t('security.tab_config'), icon: Settings },
    ];

    return (
        <div className="h-full flex flex-col p-5 gap-4 max-w-7xl mx-auto w-full">
            <div className="flex items-center justify-between">
                <h1 className="text-2xl font-bold text-gray-900 dark:text-white flex items-center gap-2">
                    <Shield className="text-blue-500" />
                    {t('security.title')}
                </h1>
                {activeTab !== 'config' && (
                    <button
                        onClick={handleRefresh}
                        className="btn btn-sm btn-ghost gap-2 text-gray-600 dark:text-gray-400"
                        title={t('security.refresh_data')}
                    >
                        <RefreshCw size={16} />
                        {t('security.refresh')}
                    </button>
                )}
            </div>

            <div className="bg-white dark:bg-base-100 rounded-xl shadow-sm border border-gray-100 dark:border-base-200 mt-2">
                <div className="flex border-b border-gray-100 dark:border-base-200">
                    {tabs.map((tab) => (
                        <button
                            key={tab.id}
                            onClick={() => setActiveTab(tab.id as any)}
                            className={`flex items-center gap-2 px-6 py-4 text-sm font-medium transition-colors relative ${activeTab === tab.id
                                ? 'text-blue-600 dark:text-blue-400'
                                : 'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200'
                                }`}
                        >
                            <tab.icon size={18} />
                            {tab.label}
                            {activeTab === tab.id && (
                                <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-blue-600 dark:bg-blue-400" />
                            )}
                        </button>
                    ))}
                </div>
                <div className="p-0">
                    {/* Content is rendered here, often components handle their own padding/layout */}
                </div>
            </div>

            <div className="flex-1 overflow-hidden flex flex-col bg-white dark:bg-base-100 rounded-xl shadow-sm border border-gray-100 dark:border-base-200">
                {renderContent()}
            </div>
        </div>
    );
};

export default Security;
