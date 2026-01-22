import { CheckCircle, Mail, Diamond, Gem, Circle } from 'lucide-react';
import { Account } from '../../types/account';
import { formatTimeRemaining } from '../../utils/format';

interface CurrentAccountProps {
    account: Account | null;
    onSwitch?: () => void;
    hideDetails?: boolean;
}

import { useTranslation } from 'react-i18next';

// Helper function to mask email - shows only first letter
const maskEmail = (email: string): string => {
    const [local, domain] = email.split('@');
    if (!domain) return 'm•••••••@••••••';
    const firstLetter = local.charAt(0).toLowerCase();
    return `${firstLetter}•••••••@••••.com`;
};

function CurrentAccount({ account, onSwitch, hideDetails = false }: CurrentAccountProps) {
    const { t } = useTranslation();
    if (!account) {
        return (
            <div className="bg-card rounded-2xl p-5 shadow-sm border border-border/50 hover:shadow-md transition-all duration-200">
                <h2 className="text-base font-semibold text-card-foreground mb-2 flex items-center gap-2">
                    <CheckCircle className="w-4 h-4 text-green-500" />
                    {t('dashboard.current_account')}
                </h2>
                <div className="text-center py-4 text-muted-foreground text-sm">
                    {t('dashboard.no_active_account')}
                </div>
            </div>
        );
    }

    const geminiProModel = account.quota?.models.find(m => m.name === 'gemini-3-pro-high');
    const geminiFlashModel = account.quota?.models.find(m => m.name === 'gemini-3-flash');
    const claudeModel = account.quota?.models.find(m => m.name === 'claude-sonnet-4-5-thinking');

    const displayEmail = hideDetails ? maskEmail(account.email) : account.email;

    return (
        <div className="bg-card rounded-2xl p-5 shadow-sm border border-border/50 h-full flex flex-col hover:shadow-md transition-all duration-200">
            <h2 className="text-base font-semibold text-card-foreground mb-3 flex items-center gap-2">
                <CheckCircle className="w-4 h-4 text-green-500" />
                {t('dashboard.current_account')}
            </h2>

            <div className="space-y-4 flex-1">
                <div className="flex items-center gap-3 mb-1">
                    <div className="flex items-center gap-2 flex-1 min-w-0">
                        <Mail className="w-3.5 h-3.5 text-muted-foreground" />
                        <span className="text-sm font-medium text-foreground truncate">{displayEmail}</span>
                    </div>
                    {/* Subscription type */}
                    {account.quota?.subscription_tier && (() => {
                        const tier = account.quota.subscription_tier.toLowerCase();
                        if (tier.includes('ultra')) {
                            return (
                                <span className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-gradient-to-r from-purple-600 to-pink-600 text-white text-[10px] font-bold shadow-sm shrink-0">
                                    <Gem className="w-2.5 h-2.5 fill-current" />
                                    ULTRA
                                </span>
                            );
                        } else if (tier.includes('pro')) {
                            return (
                                <span className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-gradient-to-r from-blue-600 to-indigo-600 text-white text-[10px] font-bold shadow-sm shrink-0">
                                    <Diamond className="w-2.5 h-2.5 fill-current" />
                                    PRO
                                </span>
                            );
                        } else {
                            return (
                                <span className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-gray-100 dark:bg-white/10 text-gray-500 dark:text-gray-400 text-[10px] font-bold shadow-sm border border-gray-200 dark:border-white/10 shrink-0">
                                    <Circle className="w-2.5 h-2.5" />
                                    FREE
                                </span>
                            );
                        }
                    })()}
                </div>

                {/* Gemini Pro 配额 */}
                {geminiProModel && (
                    <div className="space-y-1.5">
                        <div className="flex justify-between items-baseline">
                            <span className="text-xs font-medium text-gray-600 dark:text-gray-400">Gemini 3 Pro</span>
                            <div className="flex items-center gap-2">
                                <span className="text-[10px] text-gray-400 dark:text-gray-500" title={`${t('accounts.reset_time')}: ${new Date(geminiProModel.reset_time).toLocaleString()}`}>
                                    {geminiProModel.reset_time ? `R: ${formatTimeRemaining(geminiProModel.reset_time)}` : t('common.unknown')}
                                </span>
                                <span className={`text-xs font-bold ${geminiProModel.percentage >= 50 ? 'text-emerald-600 dark:text-emerald-400' :
                                    geminiProModel.percentage >= 20 ? 'text-amber-600 dark:text-amber-400' : 'text-rose-600 dark:text-rose-400'
                                    }`}>
                                    {geminiProModel.percentage}%
                                </span>
                            </div>
                        </div>
                        <div className="w-full bg-gray-100 dark:bg-base-300 rounded-full h-1.5 overflow-hidden">
                            <div
                                className={`h-full rounded-full transition-all duration-700 ${geminiProModel.percentage >= 50 ? 'bg-gradient-to-r from-emerald-400 to-emerald-500' :
                                    geminiProModel.percentage >= 20 ? 'bg-gradient-to-r from-amber-400 to-amber-500' :
                                        'bg-gradient-to-r from-rose-400 to-rose-500'
                                    }`}
                                style={{ width: `${geminiProModel.percentage}%` }}
                            ></div>
                        </div>
                    </div>
                )}

                {/* Gemini Flash 配额 */}
                {geminiFlashModel && (
                    <div className="space-y-1.5">
                        <div className="flex justify-between items-baseline">
                            <span className="text-xs font-medium text-gray-600 dark:text-gray-400">Gemini 3 Flash</span>
                            <div className="flex items-center gap-2">
                                <span className="text-[10px] text-gray-400 dark:text-gray-500" title={`${t('accounts.reset_time')}: ${new Date(geminiFlashModel.reset_time).toLocaleString()}`}>
                                    {geminiFlashModel.reset_time ? `R: ${formatTimeRemaining(geminiFlashModel.reset_time)}` : t('common.unknown')}
                                </span>
                                <span className={`text-xs font-bold ${geminiFlashModel.percentage >= 50 ? 'text-emerald-600 dark:text-emerald-400' :
                                    geminiFlashModel.percentage >= 20 ? 'text-amber-600 dark:text-amber-400' : 'text-rose-600 dark:text-rose-400'
                                    }`}>
                                    {geminiFlashModel.percentage}%
                                </span>
                            </div>
                        </div>
                        <div className="w-full bg-gray-100 dark:bg-base-300 rounded-full h-1.5 overflow-hidden">
                            <div
                                className={`h-full rounded-full transition-all duration-700 ${geminiFlashModel.percentage >= 50 ? 'bg-gradient-to-r from-emerald-400 to-emerald-500' :
                                    geminiFlashModel.percentage >= 20 ? 'bg-gradient-to-r from-amber-400 to-amber-500' :
                                        'bg-gradient-to-r from-rose-400 to-rose-500'
                                    }`}
                                style={{ width: `${geminiFlashModel.percentage}%` }}
                            ></div>
                        </div>
                    </div>
                )}

                {/* Claude 配额 */}
                {claudeModel && (
                    <div className="space-y-1.5">
                        <div className="flex justify-between items-baseline">
                            <span className="text-xs font-medium text-gray-600 dark:text-gray-400">Claude 4.5</span>
                            <div className="flex items-center gap-2">
                                <span className="text-[10px] text-gray-400 dark:text-gray-500" title={`${t('accounts.reset_time')}: ${new Date(claudeModel.reset_time).toLocaleString()}`}>
                                    {claudeModel.reset_time ? `R: ${formatTimeRemaining(claudeModel.reset_time)}` : t('common.unknown')}
                                </span>
                                <span className={`text-xs font-bold ${claudeModel.percentage >= 50 ? 'text-orange-600 dark:text-orange-400' :
                                    claudeModel.percentage >= 20 ? 'text-amber-600 dark:text-amber-400' : 'text-rose-600 dark:text-rose-400'
                                    }`}>
                                    {claudeModel.percentage}%
                                </span>
                            </div>
                        </div>
                        <div className="w-full bg-muted rounded-full h-1.5 overflow-hidden">
                            <div
                                className={`h-full rounded-full transition-all duration-700 ${claudeModel.percentage >= 50 ? 'bg-gradient-to-r from-orange-400 to-orange-500' :
                                    claudeModel.percentage >= 20 ? 'bg-gradient-to-r from-amber-400 to-amber-500' :
                                        'bg-gradient-to-r from-rose-400 to-rose-500'
                                    }`}
                                style={{ width: `${claudeModel.percentage}%` }}
                            ></div>
                        </div>
                    </div>
                )}
            </div>

            {onSwitch && (
                <div className="mt-auto pt-3">
                    <button
                        className="w-full px-3 py-1.5 text-xs text-muted-foreground border border-border rounded-lg hover:bg-muted transition-colors"
                        onClick={onSwitch}
                    >
                        {t('dashboard.switch_account')}
                    </button>
                </div>
            )}
        </div>
    );
}

export default CurrentAccount;
