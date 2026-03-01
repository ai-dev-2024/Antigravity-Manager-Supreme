import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { ThinkingBudgetConfig, ThinkingBudgetMode, ThinkingEffort } from "../../types/config";

interface ThinkingBudgetProps {
    config: ThinkingBudgetConfig;
    onChange: (config: ThinkingBudgetConfig) => void;
}

const DEFAULT_CONFIG: ThinkingBudgetConfig = {
    mode: 'auto',
    custom_value: 24576,
};

export default function ThinkingBudget({
    config = DEFAULT_CONFIG,
    onChange,
}: ThinkingBudgetProps) {
    const { t } = useTranslation();

    // 使用本地 state 管理输入值，允许临时的无效输入
    const [inputValue, setInputValue] = useState(String(config.custom_value));

    // 同步外部 config 变化
    useEffect(() => {
        setInputValue(String(config.custom_value));
    }, [config.custom_value]);

    const handleModeChange = (mode: ThinkingBudgetMode) => {
        // 切换到 adaptive 模式时，如果未设置 effort，默认设置为 high
        if (mode === 'adaptive' && !config.effort) {
            onChange({ ...config, mode, effort: 'high' });
        } else {
            onChange({ ...config, mode });
        }
    };

    const handleEffortChange = (effort: ThinkingEffort) => {
        onChange({ ...config, effort });
    };

    // 输入时只更新本地 state
    const handleInputChange = (val: string) => {
        setInputValue(val);
    };

    // 失焦时校验并提交
    const handleInputBlur = () => {
        let num = parseInt(inputValue, 10);
        if (isNaN(num) || num < 1024) num = 1024;
        if (num > 65536) num = 65536;
        setInputValue(String(num));
        onChange({ ...config, custom_value: num });
    };

    const modes: ThinkingBudgetMode[] = ['auto', 'adaptive', 'passthrough', 'custom']; // Ensure adaptive is included
    const efforts: ThinkingEffort[] = ['low', 'medium', 'high'];

    return (
        <div className="space-y-3">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 bg-blue-50/30 dark:bg-blue-900/5 border border-blue-100/50 dark:border-blue-800/20 rounded-lg px-4 py-3">
                <div className="space-y-0.5">
                    <h4 className="font-bold text-sm text-gray-900 dark:text-gray-100">
                        {t("settings.thinking_budget.title", { defaultValue: "思考预算 (Thinking Budget)" })}
                    </h4>
                    <p className="text-[10px] text-gray-500 dark:text-gray-400">
                        {t("settings.thinking_budget.mode_label", { defaultValue: "处理模式" })}
                    </p>
                </div>

                <div className="flex bg-gray-100 dark:bg-gray-800 p-1 rounded-lg">
                    {modes.map((key) => (
                        <button
                            key={key}
                            onClick={() => handleModeChange(key)}
                            className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all ${config.mode === key
                                ? 'bg-white dark:bg-gray-700 text-blue-600 dark:text-blue-400 shadow-sm'
                                : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200'
                                }`}
                        >
                            {t(`settings.thinking_budget.mode.${key}`)}
                        </button>
                    ))}
                </div>
            </div>

            {/* Mode-specific UI (Compact) */}
            <div className="px-1">
                {config.mode === 'auto' && (
                    <p className="text-[10px] text-gray-400 dark:text-gray-500 italic">
                        {t("settings.thinking_budget.auto_hint", {
                            defaultValue: "自动模式：对 Gemini/Thinking 及联网请求自动限制在 24576 以避免错误。",
                        })}
                    </p>
                )}

                {config.mode === 'passthrough' && (
                    <p className="text-[10px] text-amber-600 dark:text-amber-500/80">
                        {t("settings.thinking_budget.passthrough_warning", {
                            defaultValue: "透传：直接使用调用方原始值，不支持高值可能导致失败。",
                        })}
                    </p>
                )}

                {config.mode === 'adaptive' && (
                    <div className="flex flex-col gap-2">
                        <div className="flex items-center gap-3">
                            <span className="text-xs text-gray-500 dark:text-gray-400">
                                {t("settings.thinking_budget.effort_label", { defaultValue: "思考强度" })}:
                            </span>
                            <div className="flex bg-gray-100 dark:bg-gray-800 p-0.5 rounded-lg">
                                {efforts.map((effort) => (
                                    <button
                                        key={effort}
                                        onClick={() => handleEffortChange(effort)}
                                        className={`px-2 py-1 rounded-md text-[10px] font-medium transition-all ${config.effort === effort
                                            ? 'bg-white dark:bg-gray-700 text-purple-600 dark:text-purple-400 shadow-sm'
                                            : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200'
                                            }`}
                                    >
                                        {t(`settings.thinking_budget.effort.${effort}`)}
                                    </button>
                                ))}
                            </div>
                        </div>
                        <p className="text-[10px] text-purple-600 dark:text-purple-400/80">
                            {t("settings.thinking_budget.adaptive_hint", {
                                defaultValue: "自适应模式：由模型根据任务复杂度自动调整思考量。Claude 4.6+ 推荐使用此模式。",
                            })}
                        </p>
                    </div>
                )}


                {config.mode === 'custom' && (
                    <div className="flex items-center gap-4">
                        <div className="flex items-center gap-2">
                            <input
                                type="number"
                                value={inputValue}
                                onChange={(e) => handleInputChange(e.target.value)}
                                onBlur={handleInputBlur}
                                className="w-24 bg-white dark:bg-base-100 border border-gray-200 dark:border-gray-700 rounded-md px-2 py-1 text-xs font-mono focus:ring-1 focus:ring-blue-500 outline-none transition-all [appearance:textfield]"
                                min={1024}
                                max={65536}
                                step={1024}
                            />
                            <span className="text-[10px] text-gray-400 font-mono">TOKENS</span>
                        </div>
                        <p className="text-[10px] text-gray-500 dark:text-gray-500">
                            {t("settings.thinking_budget.custom_value_hint", {
                                defaultValue: "推荐：24576 (Flash) 或 51200 (扩展)",
                            })}
                        </p>
                    </div>
                )}
            </div>
        </div>
    );
}
