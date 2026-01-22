import { TrendingUp, Loader2 } from 'lucide-react';
import { Account } from '../../types/account';

interface BestAccountsProps {
    accounts: Account[];
    currentAccountId?: string;
    onSwitch?: (accountId: string) => void;
    isLoading?: boolean;
    hideDetails?: boolean;
    autoSwitch?: boolean;
    onAutoSwitchChange?: (enabled: boolean) => void;
}

import { useTranslation } from 'react-i18next';

// Helper function to mask email - shows only first letter
const maskEmail = (email: string): string => {
    const [local, domain] = email.split('@');
    if (!domain) return 'm•••••••@••••••';
    const firstLetter = local.charAt(0).toLowerCase();
    return `${firstLetter}•••••••@••••.com`;
};

function BestAccounts({ accounts, currentAccountId, onSwitch, isLoading = false, hideDetails = false, autoSwitch = false, onAutoSwitchChange }: BestAccountsProps) {
    const { t } = useTranslation();
    // 1. 获取按配额排序的列表 (排除当前账号)
    const geminiSorted = accounts
        .filter(a => a.id !== currentAccountId)
        .map(a => {
            const proQuota = a.quota?.models.find(m => m.name.toLowerCase() === 'gemini-3-pro-high')?.percentage || 0;
            const flashQuota = a.quota?.models.find(m => m.name.toLowerCase() === 'gemini-3-flash')?.percentage || 0;
            // 综合评分：Pro 权重更高 (70%)，Flash 权重 30%
            return {
                ...a,
                quotaVal: Math.round(proQuota * 0.7 + flashQuota * 0.3),
            };
        })
        .filter(a => a.quotaVal > 0)
        .sort((a, b) => b.quotaVal - a.quotaVal);

    const claudeSorted = accounts
        .filter(a => a.id !== currentAccountId)
        .map(a => ({
            ...a,
            quotaVal: a.quota?.models.find(m => m.name.toLowerCase().includes('claude'))?.percentage || 0,
        }))
        .filter(a => a.quotaVal > 0)
        .sort((a, b) => b.quotaVal - a.quotaVal);

    let bestGemini = geminiSorted[0];
    let bestClaude = claudeSorted[0];

    // 2. 如果推荐是同一个账号，且有其他选择，尝试寻找最优的"不同账号"组合
    if (bestGemini && bestClaude && bestGemini.id === bestClaude.id) {
        const nextGemini = geminiSorted[1];
        const nextClaude = claudeSorted[1];

        // 方案A: 保持 Gemini 最优，换 Claude 次优
        // 方案B: 换 Gemini 次优，保持 Claude 最优
        // 比较标准：两者配额之和最大化 (或者优先保住 100% 的那个)

        const scoreA = bestGemini.quotaVal + (nextClaude?.quotaVal || 0);
        const scoreB = (nextGemini?.quotaVal || 0) + bestClaude.quotaVal;

        if (nextClaude && (!nextGemini || scoreA >= scoreB)) {
            // 选方案A：换 Claude
            bestClaude = nextClaude;
        } else if (nextGemini) {
            // 选方案B：换 Gemini
            bestGemini = nextGemini;
        }
        // 如果都没有次优解（例如只有一个账号），则保持原样
    }

    // 构造最终用于显示的视图模型 (兼容原有渲染逻辑)
    const bestGeminiRender = bestGemini ? { ...bestGemini, geminiQuota: bestGemini.quotaVal } : undefined;
    const bestClaudeRender = bestClaude ? { ...bestClaude, claudeQuota: bestClaude.quotaVal } : undefined;

    return (
        <div className="bg-card rounded-2xl p-5 shadow-sm border border-border/50 h-full flex flex-col hover:shadow-md transition-all duration-200">
            <h2 className="text-base font-semibold text-card-foreground mb-3 flex items-center gap-2">
                <TrendingUp className="w-4 h-4 text-blue-500" />
                {t('dashboard.best_accounts')}
            </h2>

            <div className="space-y-2 flex-1">
                {/* Gemini 最佳 */}
                {bestGeminiRender && (
                    <div className="flex items-center justify-between p-2.5 bg-green-500/10 rounded-lg border border-green-500/20">
                        <div className="flex-1 min-w-0">
                            <div className="text-[10px] text-green-500 font-medium mb-0.5">{t('dashboard.for_gemini')}</div>
                            <div className="font-medium text-sm text-card-foreground truncate">
                                {hideDetails ? maskEmail(bestGeminiRender.email) : bestGeminiRender.email}
                            </div>
                        </div>
                        <div className="ml-2 px-2 py-0.5 bg-green-500 text-white text-xs font-semibold rounded-full">
                            {bestGeminiRender.geminiQuota}%
                        </div>
                    </div>
                )}

                {/* Claude 最佳 */}
                {bestClaudeRender && (
                    <div className="flex items-center justify-between p-2.5 bg-orange-500/10 rounded-lg border border-orange-500/20">
                        <div className="flex-1 min-w-0">
                            <div className="text-[10px] text-orange-500 font-medium mb-0.5">{t('dashboard.for_claude')}</div>
                            <div className="font-medium text-sm text-card-foreground truncate">
                                {hideDetails ? maskEmail(bestClaudeRender.email) : bestClaudeRender.email}
                            </div>
                        </div>
                        <div className="ml-2 px-2 py-0.5 bg-orange-500 text-white text-xs font-semibold rounded-full">
                            {bestClaudeRender.claudeQuota}%
                        </div>
                    </div>
                )}

                {(!bestGeminiRender && !bestClaudeRender) && (
                    <div className="text-center py-4 text-muted-foreground text-sm">
                        {t('accounts.no_data')}
                    </div>
                )}
            </div>

            {(bestGeminiRender || bestClaudeRender) && onSwitch && (
                <div className="mt-auto pt-3 space-y-2">
                    {/* Switch to Best (overall - higher quota wins) */}
                    <button
                        className="w-full px-3 py-1.5 bg-primary text-primary-foreground text-xs font-medium rounded-lg hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                        disabled={isLoading}
                        onClick={() => {
                            // Switch to whichever has higher quota
                            let targetId = bestGeminiRender?.id;
                            if (bestClaudeRender && (!bestGeminiRender || bestClaudeRender.claudeQuota > bestGeminiRender.geminiQuota)) {
                                targetId = bestClaudeRender.id;
                            }
                            if (onSwitch && targetId) {
                                onSwitch(targetId);
                            }
                        }}
                    >
                        {isLoading && <Loader2 className="w-3 h-3 animate-spin" />}
                        {t('dashboard.switch_best')}
                    </button>

                    {/* Two-column layout for Claude and Gemini */}
                    <div className="grid grid-cols-2 gap-2">
                        {/* Switch Best Claude */}
                        {bestClaudeRender && (
                            <button
                                className="px-3 py-1.5 bg-orange-500/20 text-orange-600 dark:text-orange-400 text-xs font-medium rounded-lg hover:bg-orange-500/30 transition-colors border border-orange-500/30 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-1"
                                disabled={isLoading}
                                onClick={() => {
                                    if (onSwitch && bestClaudeRender.id) {
                                        onSwitch(bestClaudeRender.id);
                                    }
                                }}
                            >
                                {isLoading && <Loader2 className="w-3 h-3 animate-spin" />}
                                {t('dashboard.switch_best_claude', { defaultValue: 'Best Claude' })}
                            </button>
                        )}

                        {/* Switch Best Gemini */}
                        {bestGeminiRender && (
                            <button
                                className="px-3 py-1.5 bg-green-500/20 text-green-600 dark:text-green-400 text-xs font-medium rounded-lg hover:bg-green-500/30 transition-colors border border-green-500/30 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-1"
                                disabled={isLoading}
                                onClick={() => {
                                    if (onSwitch && bestGeminiRender.id) {
                                        onSwitch(bestGeminiRender.id);
                                    }
                                }}
                            >
                                {isLoading && <Loader2 className="w-3 h-3 animate-spin" />}
                                {t('dashboard.switch_best_gemini', { defaultValue: 'Best Gemini' })}
                            </button>
                        )}
                    </div>

                    {/* Auto-Switch Toggle */}
                    {onAutoSwitchChange && (
                        <div className="mt-3 p-2.5 bg-blue-500/10 border border-blue-500/20 rounded-lg">
                            <label className="flex items-center justify-between cursor-pointer">
                                <div className="flex items-center gap-2">
                                    <span className="text-xs">🔄</span>
                                    <span className="text-xs font-medium text-card-foreground">
                                        {t('dashboard.auto_switch', { defaultValue: 'Auto-Switch' })}
                                    </span>
                                </div>
                                <div className="relative">
                                    <input
                                        type="checkbox"
                                        className="sr-only peer"
                                        checked={autoSwitch}
                                        onChange={(e) => onAutoSwitchChange(e.target.checked)}
                                    />
                                    <div className="w-9 h-5 bg-muted rounded-full peer peer-checked:bg-blue-500 transition-colors"></div>
                                    <div className="absolute left-0.5 top-0.5 w-4 h-4 bg-white rounded-full shadow transition-transform peer-checked:translate-x-4"></div>
                                </div>
                            </label>
                            <p className="text-[10px] text-muted-foreground mt-1.5">
                                {t('dashboard.auto_switch_desc', { defaultValue: 'Auto-switch to another account when quota depletes, then relaunch Antigravity' })}
                            </p>
                        </div>
                    )}
                </div>
            )}
        </div>
    );

}

export default BestAccounts;
