import React from 'react';
import { useTranslation } from 'react-i18next';
import { Terminal } from 'lucide-react';
import { useDebugConsole } from '../../stores/useDebugConsole';
import { cn } from '../../utils/cn';

const DebugConsoleButton: React.FC = () => {
    const { t } = useTranslation();
    const { isEnabled, isOpen, toggle, enable, logs } = useDebugConsole();

    const handleClick = () => {
        if (!isEnabled) {
            enable();
        }
        toggle();
    };

    const errorCount = logs.filter(l => l.level === 'ERROR').length;
    const warnCount = logs.filter(l => l.level === 'WARN').length;

    return (
        <button
            onClick={handleClick}
            className={cn(
                "relative flex items-center gap-1.5 px-2 py-1 rounded-lg text-xs font-medium transition-all",
                isOpen
                    ? "bg-green-500/20 text-green-400 border border-green-500/30"
                    : "bg-zinc-800/50 text-zinc-400 hover:text-white hover:bg-zinc-700/50 border border-transparent"
            )}
            title={t('debug_console.toggle', { defaultValue: 'Toggle Debug Console' })}
        >
            <Terminal size={14} className={isOpen ? "text-green-400" : ""} />
            <span className="hidden sm:inline">Console</span>

            {/* Error badge */}
            {errorCount > 0 && (
                <span className="absolute -top-1 -right-1 min-w-4 h-4 flex items-center justify-center px-1 rounded-full bg-red-500 text-white text-[9px] font-bold">
                    {errorCount > 99 ? '99+' : errorCount}
                </span>
            )}

            {/* Warn badge (only if no errors) */}
            {errorCount === 0 && warnCount > 0 && (
                <span className="absolute -top-1 -right-1 min-w-4 h-4 flex items-center justify-center px-1 rounded-full bg-amber-500 text-white text-[9px] font-bold">
                    {warnCount > 99 ? '99+' : warnCount}
                </span>
            )}
        </button>
    );
};

export default DebugConsoleButton;
