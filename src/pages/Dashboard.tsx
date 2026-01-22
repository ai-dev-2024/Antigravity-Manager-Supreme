import { useEffect, useMemo, useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { Users, Sparkles, Bot, AlertTriangle, ArrowRight, Download, RefreshCw, Eye, EyeOff } from 'lucide-react';
import { useAccountStore } from '../stores/useAccountStore';
import CurrentAccount from '../components/dashboard/CurrentAccount';
import BestAccounts from '../components/dashboard/BestAccounts';
import AddAccountDialog from '../components/accounts/AddAccountDialog';
import { save } from '@tauri-apps/plugin-dialog';
import { request as invoke } from '../utils/request';
import { showToast } from '../components/common/ToastContainer';
import { Account } from '../types/account';
import { useConfigStore } from '../stores/useConfigStore';

function Dashboard() {
    const { t } = useTranslation();
    const navigate = useNavigate();
    const {
        accounts,
        currentAccount,
        fetchAccounts,
        fetchCurrentAccount,
        switchAccount,
        addAccount,
        refreshQuota,
        loading
    } = useAccountStore();

    const { config, saveConfig } = useConfigStore();

    useEffect(() => {
        fetchAccounts();
        fetchCurrentAccount();
    }, []);

    // Auto-refresh quotas every 5 minutes
    useEffect(() => {
        const REFRESH_INTERVAL = 5 * 60 * 1000; // 5 minutes

        const refreshAllQuotas = async () => {
            if (accounts.length === 0) return;
            try {
                // Silently refresh all account quotas in the background
                await invoke('batch_refresh_quotas');
                await fetchAccounts(); // Reload updated data
                console.log('[Dashboard] Auto-refreshed quotas for all accounts');
            } catch (error) {
                console.error('[Dashboard] Auto-refresh failed:', error);
            }
        };

        const interval = setInterval(refreshAllQuotas, REFRESH_INTERVAL);
        return () => clearInterval(interval);
    }, [accounts.length]);

    // Auto-switch: Monitor quota and switch accounts when depleted
    const lastAutoSwitchRef = useRef<number>(0);
    useEffect(() => {
        if (!config?.auto_switch || !currentAccount) return;

        const checkQuotaAndSwitch = async () => {
            // Prevent rapid switching (debounce 30 seconds)
            const now = Date.now();
            if (now - lastAutoSwitchRef.current < 30000) return;

            // Get current account's quotas for both Claude and Gemini
            const claudeQuota = currentAccount.quota?.models.find(
                m => m.name.toLowerCase().includes('claude')
            )?.percentage || 0;

            const geminiQuota = currentAccount.quota?.models.find(
                m => m.name.toLowerCase() === 'gemini-3-pro-high'
            )?.percentage || 0;

            // Check if either model is depleted
            const claudeDepleted = claudeQuota <= 0;
            const geminiDepleted = geminiQuota <= 0;

            if (!claudeDepleted && !geminiDepleted) return; // Both have quota, no need to switch

            console.log(`[Auto-Switch] Quota check: Claude=${claudeQuota}%, Gemini=${geminiQuota}%`);

            // Find best account based on which model is depleted
            // Priority: If Claude is depleted, find best Claude account
            // If only Gemini is depleted, find best Gemini account
            let targetAccount = null;
            let switchReason = '';

            if (claudeDepleted) {
                // Find best Claude account (excluding current)
                const bestClaude = accounts
                    .filter(a => a.id !== currentAccount.id)
                    .map(a => ({
                        ...a,
                        claudeQ: a.quota?.models.find(m => m.name.toLowerCase().includes('claude'))?.percentage || 0
                    }))
                    .filter(a => a.claudeQ > 0)
                    .sort((a, b) => b.claudeQ - a.claudeQ)[0];

                if (bestClaude) {
                    targetAccount = bestClaude;
                    switchReason = `Claude quota depleted, switching to ${bestClaude.email} (${bestClaude.claudeQ}% Claude)`;
                }
            } else if (geminiDepleted) {
                // Find best Gemini account (excluding current)
                const bestGemini = accounts
                    .filter(a => a.id !== currentAccount.id)
                    .map(a => ({
                        ...a,
                        geminiQ: a.quota?.models.find(m => m.name.toLowerCase() === 'gemini-3-pro-high')?.percentage || 0
                    }))
                    .filter(a => a.geminiQ > 0)
                    .sort((a, b) => b.geminiQ - a.geminiQ)[0];

                if (bestGemini) {
                    targetAccount = bestGemini;
                    switchReason = `Gemini quota depleted, switching to ${bestGemini.email} (${bestGemini.geminiQ}% Gemini)`;
                }
            }

            if (targetAccount) {
                lastAutoSwitchRef.current = now;
                console.log(`[Auto-Switch] ${switchReason}`);
                showToast(t('dashboard.auto_switch_switching', { defaultValue: switchReason }), 'info');

                try {
                    await switchAccount(targetAccount.id);
                    // Relaunch Antigravity
                    await invoke('launch_antigravity');
                    showToast(t('dashboard.auto_switch_success', { defaultValue: 'Auto-switched and relaunched Antigravity!' }), 'success');
                } catch (error) {
                    console.error('[Auto-Switch] Failed:', error);
                    showToast(`Auto-switch failed: ${error}`, 'error');
                }
            }
        };

        // Check every 30 seconds
        const interval = setInterval(checkQuotaAndSwitch, 30000);
        checkQuotaAndSwitch(); // Initial check
        return () => clearInterval(interval);
    }, [config?.auto_switch, currentAccount, accounts, switchAccount, t]);

    // Calculate statistics
    const stats = useMemo(() => {
        const geminiQuotas = accounts
            .map(a => a.quota?.models.find(m => m.name.toLowerCase() === 'gemini-3-pro-high')?.percentage || 0)
            .filter(q => q > 0);

        const geminiImageQuotas = accounts
            .map(a => a.quota?.models.find(m => m.name.toLowerCase() === 'gemini-3-pro-image')?.percentage || 0)
            .filter(q => q > 0);

        const claudeQuotas = accounts
            .map(a => a.quota?.models.find(m => m.name.toLowerCase() === 'claude-sonnet-4-5')?.percentage || 0)
            .filter(q => q > 0);

        const lowQuotaCount = accounts.filter(a => {
            const gemini = a.quota?.models.find(m => m.name.toLowerCase() === 'gemini-3-pro-high')?.percentage || 0;
            const claude = a.quota?.models.find(m => m.name.toLowerCase() === 'claude-sonnet-4-5')?.percentage || 0;
            return gemini < 20 || claude < 20;
        }).length;

        return {
            total: accounts.length,
            avgGemini: geminiQuotas.length > 0
                ? Math.round(geminiQuotas.reduce((a, b) => a + b, 0) / geminiQuotas.length)
                : 0,
            avgGeminiImage: geminiImageQuotas.length > 0
                ? Math.round(geminiImageQuotas.reduce((a, b) => a + b, 0) / geminiImageQuotas.length)
                : 0,
            avgClaude: claudeQuotas.length > 0
                ? Math.round(claudeQuotas.reduce((a, b) => a + b, 0) / claudeQuotas.length)
                : 0,
            lowQuota: lowQuotaCount,
        };
    }, [accounts]);

    const isSwitchingRef = useRef(false);

    const handleSwitch = async (accountId: string) => {
        if (loading || isSwitchingRef.current) return;

        isSwitchingRef.current = true;
        console.log('[Dashboard] handleSwitch called for', accountId);
        try {
            await switchAccount(accountId);
            showToast(t('dashboard.toast.switch_success'), 'success');

            // Launch Antigravity with the new account
            try {
                await invoke('launch_antigravity');
                showToast(t('dashboard.toast.antigravity_launched', { defaultValue: 'Antigravity relaunched with new account!' }), 'success');
            } catch (launchError) {
                console.error('Launch Antigravity failed:', launchError);
                showToast(`${t('dashboard.toast.launch_error', { defaultValue: 'Failed to launch Antigravity' })}: ${launchError}`, 'warning');
            }
        } catch (error) {
            console.error('Switch account failed:', error);
            showToast(`${t('dashboard.toast.switch_error')}: ${error}`, 'error');
        } finally {
            setTimeout(() => {
                isSwitchingRef.current = false;
            }, 1000);
        }
    };

    const handleAddAccount = async (email: string, refreshToken: string) => {
        await addAccount(email, refreshToken);
        await fetchAccounts(); // Refresh list
    };

    const [isRefreshing, setIsRefreshing] = useState(false);
    const [hideDetails, setHideDetails] = useState(false);

    const handleRefreshCurrent = async () => {
        if (!currentAccount) return;

        setIsRefreshing(true);
        try {
            await refreshQuota(currentAccount.id);
            // Refresh latest data after success
            await fetchCurrentAccount();
            showToast(t('dashboard.toast.refresh_success'), 'success');
        } catch (error) {
            console.error('[Dashboard] Refresh failed:', error);
            showToast(`${t('dashboard.toast.refresh_error')}: ${error}`, 'error');
        } finally {
            setIsRefreshing(false);
        }
    };

    const exportAccountsToJson = async (accountsToExport: Account[]) => {
        try {
            if (accountsToExport.length === 0) {
                showToast(t('dashboard.toast.export_no_accounts'), 'warning');
                return;
            }

            const path = await save({
                filters: [{
                    name: 'JSON',
                    extensions: ['json']
                }],
                defaultPath: `antigravity_accounts_${new Date().toISOString().split('T')[0]}.json`
            });

            if (!path) return;

            const exportData = accountsToExport.map(acc => ({
                email: acc.email,
                refresh_token: acc.token.refresh_token
            }));

            const content = JSON.stringify(exportData, null, 2);

            await invoke('save_text_file', { path, content });

            showToast(t('dashboard.toast.export_success', { path }), 'success');
        } catch (error) {
            console.error('Export failed:', error);
            showToast(`${t('dashboard.toast.export_error')}: ${error}`, 'error');
        }
    };

    const handleExport = () => {
        exportAccountsToJson(accounts);
    };

    return (
        <div className="h-full w-full overflow-y-auto">
            <div
                className="p-5 space-y-4 max-w-7xl mx-auto"
                onMouseMove={() => console.log('Mouse moving over Dashboard')}
                style={{ position: 'relative', zIndex: 1 }}
            >
                {/* Action Buttons */}
                <div className="flex justify-between items-center">
                    {/* Hide Details Toggle */}
                    <button
                        onClick={() => setHideDetails(!hideDetails)}
                        className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all bg-muted/50 text-muted-foreground hover:bg-muted hover:text-foreground"
                        title={hideDetails ? 'Show account details' : 'Hide account details'}
                    >
                        {hideDetails ? <EyeOff className="w-3.5 h-3.5" /> : <Eye className="w-3.5 h-3.5" />}
                        {hideDetails ? 'Show Details' : 'Hide Details'}
                    </button>
                    <div className="flex gap-2">
                        <AddAccountDialog onAdd={handleAddAccount} />
                        <button
                            className={`inline-flex items-center justify-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all duration-200 ease-elegant ${isRefreshing || !currentAccount
                                ? 'bg-primary/50 text-primary-foreground cursor-not-allowed'
                                : 'bg-primary text-primary-foreground shadow-sm hover:bg-primary/90 hover:shadow-md active:scale-95'
                                }`}
                            onClick={handleRefreshCurrent}
                            disabled={isRefreshing || !currentAccount}
                        >
                            <RefreshCw className={`w-3.5 h-3.5 ${isRefreshing ? 'animate-spin' : ''}`} />
                            {isRefreshing ? t('dashboard.refreshing') : t('dashboard.refresh_quota')}
                        </button>
                    </div>
                </div>

                {/* Stat Cards */}
                <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
                    <div className="bg-card rounded-2xl p-4 shadow-sm border border-border/50 hover:shadow-md transition-all duration-200 ease-elegant hover:scale-[1.02]">
                        <div className="flex items-center justify-between mb-2">
                            <div className="p-2 bg-blue-500/10 rounded-xl">
                                <Users className="w-4 h-4 text-blue-500" />
                            </div>
                        </div>
                        <div className="text-2xl font-bold text-card-foreground mb-0.5">{stats.total}</div>
                        <div className="text-xs text-muted-foreground">{t('dashboard.total_accounts')}</div>
                    </div>

                    <div className="bg-card rounded-2xl p-4 shadow-sm border border-border/50 hover:shadow-md transition-all duration-200 ease-elegant hover:scale-[1.02]">
                        <div className="flex items-center justify-between mb-2">
                            <div className="p-2 bg-green-500/10 rounded-xl">
                                <Sparkles className="w-4 h-4 text-green-500" />
                            </div>
                        </div>
                        <div className="text-2xl font-bold text-card-foreground mb-0.5">{stats.avgGemini}%</div>
                        <div className="text-xs text-muted-foreground">{t('dashboard.avg_gemini')}</div>
                        {stats.avgGemini > 0 && (
                            <div className={`text-[10px] mt-1 ${stats.avgGemini >= 50 ? 'text-green-500' : 'text-orange-500'}`}>
                                {stats.avgGemini >= 50 ? t('dashboard.quota_sufficient') : t('dashboard.quota_low')}
                            </div>
                        )}
                    </div>

                    <div className="bg-card rounded-2xl p-4 shadow-sm border border-border/50 hover:shadow-md transition-all duration-200 ease-elegant hover:scale-[1.02]">
                        <div className="flex items-center justify-between mb-2">
                            <div className="p-2 bg-purple-500/10 rounded-xl">
                                <Sparkles className="w-4 h-4 text-purple-500" />
                            </div>
                        </div>
                        <div className="text-2xl font-bold text-card-foreground mb-0.5">{stats.avgGeminiImage}%</div>
                        <div className="text-xs text-muted-foreground">{t('dashboard.avg_gemini_image')}</div>
                        {stats.avgGeminiImage > 0 && (
                            <div className={`text-[10px] mt-1 ${stats.avgGeminiImage >= 50 ? 'text-green-500' : 'text-orange-500'}`}>
                                {stats.avgGeminiImage >= 50 ? t('dashboard.quota_sufficient') : t('dashboard.quota_low')}
                            </div>
                        )}
                    </div>

                    <div className="bg-card rounded-2xl p-4 shadow-sm border border-border/50 hover:shadow-md transition-all duration-200 ease-elegant hover:scale-[1.02]">
                        <div className="flex items-center justify-between mb-2">
                            <div className="p-2 bg-orange-500/10 rounded-xl">
                                <Bot className="w-4 h-4 text-orange-500" />
                            </div>
                        </div>
                        <div className="text-2xl font-bold text-card-foreground mb-0.5">{stats.avgClaude}%</div>
                        <div className="text-xs text-muted-foreground">{t('dashboard.avg_claude')}</div>
                        {stats.avgClaude > 0 && (
                            <div className={`text-[10px] mt-1 ${stats.avgClaude >= 50 ? 'text-green-500' : 'text-orange-500'}`}>
                                {stats.avgClaude >= 50 ? t('dashboard.quota_sufficient') : t('dashboard.quota_low')}
                            </div>
                        )}
                    </div>

                    <div className="bg-card rounded-2xl p-4 shadow-sm border border-border/50 hover:shadow-md transition-all duration-200 ease-elegant hover:scale-[1.02]">
                        <div className="flex items-center justify-between mb-2">
                            <div className="p-2 bg-orange-500/10 rounded-xl">
                                <AlertTriangle className="w-4 h-4 text-orange-500" />
                            </div>
                        </div>
                        <div className="text-2xl font-bold text-card-foreground mb-0.5">{stats.lowQuota}</div>
                        <div className="text-xs text-muted-foreground">{t('dashboard.low_quota_accounts')}</div>
                        <div className="text-[10px] text-muted-foreground mt-1">{t('dashboard.quota_desc')}</div>
                    </div>
                </div>

                {/* Two-column layout */}
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <CurrentAccount
                        account={currentAccount}
                        onSwitch={() => navigate('/accounts')}
                        hideDetails={hideDetails}
                    />
                    <BestAccounts
                        accounts={accounts}
                        currentAccountId={currentAccount?.id}
                        onSwitch={handleSwitch}
                        isLoading={loading}
                        hideDetails={hideDetails}
                        autoSwitch={config?.auto_switch}
                        onAutoSwitchChange={async (enabled) => {
                            if (config) {
                                await saveConfig({ ...config, auto_switch: enabled });
                                showToast(enabled
                                    ? t('dashboard.auto_switch_enabled', { defaultValue: 'Auto-Switch enabled' })
                                    : t('dashboard.auto_switch_disabled', { defaultValue: 'Auto-Switch disabled' }),
                                    'success'
                                );
                            }
                        }}
                    />
                </div>

                {/* Quick Links */}
                <div className="grid grid-cols-2 gap-3">
                    <button
                        className="bg-card rounded-2xl p-3 shadow-sm border border-border/50 hover:border-primary/50 hover:shadow-md transition-all duration-200 ease-elegant flex items-center justify-between group hover:scale-[1.01]"
                        onClick={() => navigate('/accounts')}
                    >
                        <span className="text-card-foreground font-medium text-sm">{t('dashboard.view_all_accounts')}</span>
                        <ArrowRight className="w-4 h-4 text-muted-foreground group-hover:text-primary group-hover:translate-x-1 transition-all duration-200" />
                    </button>
                    <button
                        className="bg-card rounded-2xl p-3 shadow-sm border border-border/50 hover:border-primary/50 hover:shadow-md transition-all duration-200 ease-elegant flex items-center justify-between group hover:scale-[1.01]"
                        onClick={handleExport}
                    >
                        <span className="text-card-foreground font-medium text-sm">{t('dashboard.export_data')}</span>
                        <Download className="w-4 h-4 text-muted-foreground group-hover:text-primary transition-all duration-200" />
                    </button>
                </div>
            </div>
        </div>
    );
}

export default Dashboard;
