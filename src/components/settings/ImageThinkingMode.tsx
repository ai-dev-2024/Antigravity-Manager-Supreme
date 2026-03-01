import { useTranslation } from "react-i18next";
import { Image } from "lucide-react";

interface ImageThinkingModeProps {
    value?: 'enabled' | 'disabled';
    onChange: (value: 'enabled' | 'disabled') => void;
}

export default function ImageThinkingMode({
    value = 'enabled',
    onChange,
}: ImageThinkingModeProps) {
    const { t } = useTranslation();

    const options = [
        { value: 'enabled', label: 'enabled', desc: 'enabled_desc' },
        { value: 'disabled', label: 'disabled', desc: 'disabled_desc' },
    ] as const;

    return (
        <div className="space-y-3">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 bg-pink-50/30 dark:bg-pink-900/5 border border-pink-100/50 dark:border-pink-800/20 rounded-lg px-4 py-3">
                <div className="flex items-center gap-3">
                    <div className="p-1.5 bg-pink-100 dark:bg-pink-900/30 rounded text-pink-600 dark:text-pink-400">
                        <Image size={18} />
                    </div>
                    <div className="space-y-0.5">
                        <h4 className="font-bold text-sm text-gray-900 dark:text-gray-100">
                            {t("settings.image_thinking_mode.title", { defaultValue: "图像思维模式 (Image Thinking Mode)" })}
                        </h4>
                        <p className="text-[10px] text-gray-500 dark:text-gray-400">
                            {t("settings.image_thinking_mode.hint", { defaultValue: "影响画质与生成流程" })}
                        </p>
                    </div>
                </div>

                <div className="flex bg-gray-100 dark:bg-gray-800 p-1 rounded-lg">
                    {options.map((option) => (
                        <button
                            key={option.value}
                            onClick={() => onChange(option.value)}
                            className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all ${value === option.value
                                ? 'bg-white dark:bg-gray-700 text-pink-600 dark:text-pink-400 shadow-sm'
                                : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200'
                                }`}
                        >
                            {t(`settings.image_thinking_mode.options.${option.label}`, {
                                defaultValue: option.value === 'enabled' ? "开启" : "关闭"
                            })}
                        </button>
                    ))}
                </div>
            </div>

            <div className="px-1">
                <p className="text-[10px] text-gray-400 dark:text-gray-500 italic leading-relaxed">
                    {value === 'enabled'
                        ? t("settings.image_thinking_mode.options.enabled_desc", { defaultValue: "开启：保留思维链，返回草图 + 成品双图。" })
                        : t("settings.image_thinking_mode.options.disabled_desc", { defaultValue: "关闭：禁用思维链，直接生成单张超清图片（画质优先）。" })
                    }
                </p>
            </div>
        </div>
    );
}
