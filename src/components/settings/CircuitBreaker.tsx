import { useTranslation } from "react-i18next";
import { CircuitBreakerConfig } from "../../types/config";
import { ShieldAlert, Trash2, Plus, Minus, Clock } from "lucide-react";

interface CircuitBreakerProps {
    config: CircuitBreakerConfig;
    onChange: (config: CircuitBreakerConfig) => void;
    onClearRateLimits?: () => void;
}

export default function CircuitBreaker({
    config,
    onChange,
    onClearRateLimits,
}: CircuitBreakerProps) {
    const { t } = useTranslation();

    const handleLevelChange = (index: number, val: string) => {
        let num = parseInt(val, 10);
        if (isNaN(num)) num = 0;

        const newSteps = [...config.backoff_steps];
        newSteps[index] = Math.max(0, num);
        onChange({ ...config, backoff_steps: newSteps });
    };

    const addLevel = () => {
        const lastVal = config.backoff_steps[config.backoff_steps.length - 1] || 60;
        onChange({
            ...config,
            backoff_steps: [...config.backoff_steps, lastVal * 2],
        });
    };

    const removeLevel = (index: number) => {
        if (config.backoff_steps.length <= 1) return;
        const newSteps = config.backoff_steps.filter((_, i) => i !== index);
        onChange({ ...config, backoff_steps: newSteps });
    };

    const getStepColorCls = (index: number) => {
        if (index === 0) return "border-yellow-200 dark:border-yellow-700/50 bg-yellow-50/30 dark:bg-yellow-900/10";
        if (index === 1) return "border-orange-200 dark:border-orange-700/50 bg-orange-50/30 dark:bg-orange-900/10";
        if (index === 2) return "border-red-200 dark:border-red-700/50 bg-red-50/30 dark:bg-red-900/10";
        return "border-rose-200 dark:border-rose-700/50 bg-rose-50/30 dark:bg-rose-900/10";
    };

    return (
        <div className="space-y-6">
            <div className="bg-orange-50/50 dark:bg-orange-900/10 border border-orange-100 dark:border-orange-800/30 rounded-lg p-4">
                <div className="flex gap-3">
                    <ShieldAlert className="w-5 h-5 text-orange-500 shrink-0 mt-0.5" />
                    <div className="space-y-1">
                        <h4 className="font-medium text-sm text-gray-900 dark:text-gray-100">
                            {t("proxy.config.circuit_breaker.title", { defaultValue: "Adaptive Circuit Breaker" })}
                        </h4>
                        <p className="text-xs text-gray-500 dark:text-gray-400 leading-relaxed">
                            {t("proxy.config.circuit_breaker.tooltip", {
                                defaultValue: "Automatically increases lockout duration for accounts that repeatedly fail with quota exhaustion. This prevents wasting API calls on dead accounts while allowing transient errors to recover quickly.",
                            })}
                        </p>
                    </div>
                </div>
            </div>

            <div className="space-y-4">
                <div className="flex justify-between items-center">
                    <label className="text-sm font-bold text-gray-700 dark:text-gray-300 flex items-center gap-2">
                        <Clock className="w-4 h-4 text-blue-500" />
                        {t("proxy.config.circuit_breaker.backoff_levels", { defaultValue: "Backoff Levels (Seconds)" })}
                    </label>
                    <button
                        onClick={(e) => { e.stopPropagation(); addLevel(); }}
                        className="btn btn-xs btn-ghost text-blue-500 hover:bg-blue-50 dark:hover:bg-blue-900/20 gap-1 h-7 min-h-0 px-2 rounded-md border border-blue-200 dark:border-blue-800/50 shadow-sm"
                    >
                        <Plus size={14} />
                        {t("common.add", { defaultValue: "Add" })}
                    </button>
                </div>

                <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                    {config.backoff_steps.map((seconds, idx) => (
                        <div
                            key={idx}
                            className={`p-3 rounded-xl border transition-all hover:shadow-sm group relative ${getStepColorCls(idx)}`}
                        >
                            <div className="flex flex-col gap-2">
                                <div className="flex justify-between items-center">
                                    <span className="text-[10px] font-bold uppercase tracking-wider opacity-60">
                                        {t("proxy.config.circuit_breaker.level", { level: idx + 1, defaultValue: `Lv ${idx + 1}` })}
                                    </span>
                                    {config.backoff_steps.length > 1 && (
                                        <button
                                            onClick={(e) => { e.stopPropagation(); removeLevel(idx); }}
                                            className="opacity-0 group-hover:opacity-100 p-1 hover:text-red-500 transition-opacity"
                                            title={t("common.delete", { defaultValue: "Delete" })}
                                        >
                                            <Minus size={12} />
                                        </button>
                                    )}
                                </div>
                                <div className="relative">
                                    <input
                                        type="number"
                                        value={seconds}
                                        onChange={(e) => handleLevelChange(idx, e.target.value)}
                                        className="w-full bg-white dark:bg-base-100 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-1.5 text-sm font-mono focus:ring-2 focus:ring-blue-500/20 outline-none transition-all [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
                                        min="0"
                                    />
                                    <span className="absolute right-3 top-1/2 -translate-y-1/2 text-[10px] font-bold opacity-30 select-none pointer-events-none">S</span>
                                </div>
                            </div>
                        </div>
                    ))}
                </div>
            </div>

            {onClearRateLimits && (
                <div className="pt-2 border-t border-gray-100 dark:border-gray-800">
                    <button
                        onClick={onClearRateLimits}
                        className="btn btn-sm btn-ghost text-gray-500 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 gap-2 w-full justify-start h-auto py-2 px-1"
                    >
                        <Trash2 className="w-4 h-4" />
                        <span className="text-xs">
                            {t("proxy.config.circuit_breaker.clear_records", { defaultValue: "Clear All Rate Limit Records" })}
                        </span>
                    </button>
                </div>
            )}
        </div>
    );
}
