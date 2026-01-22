import { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import {
    Power,
    Copy,
    RefreshCw,
    CheckCircle,
    Settings,
    Target,
    Plus,
    Terminal,
    Code,
    Image as ImageIcon,
    BrainCircuit,
    Sparkles,
    Zap,
    Cpu,
    Puzzle,
    Wind,
    ArrowRight,
    Trash2,
    Layers,
    Activity
} from 'lucide-react';
import { AppConfig, ProxyConfig, StickySessionConfig } from '../types/config';
import HelpTooltip from '../components/common/HelpTooltip';
import ModalDialog from '../components/common/ModalDialog';
import { showToast } from '../components/common/ToastContainer';

interface ProxyStatus {
    running: boolean;
    port: number;
    base_url: string;
    active_accounts: number;
}


interface CollapsibleCardProps {
    title: string;
    icon: React.ReactNode;
    enabled?: boolean;
    onToggle?: (enabled: boolean) => void;
    children: React.ReactNode;
    defaultExpanded?: boolean;
    rightElement?: React.ReactNode;
}

function CollapsibleCard({
    title,
    icon,
    enabled,
    onToggle,
    children,
    defaultExpanded = false,
    rightElement
}: CollapsibleCardProps) {
    const [isExpanded, setIsExpanded] = useState(defaultExpanded);
    const { t } = useTranslation();

    return (
        <div className="bg-card rounded-xl shadow-sm border border-border overflow-hidden transition-all duration-200 hover:shadow-md">
            <div
                className="px-5 py-4 flex items-center justify-between cursor-pointer bg-gray-50/50 dark:bg-gradient-to-b from-[#2a2a2a] to-[#1a1a1a] hover:bg-gray-50 dark:hover:from-[#2f2f2f] dark:hover:to-[#1f1f1f] transition-colors"
                onClick={(e) => {
                    // Prevent toggle when clicking the switch or right element
                    if ((e.target as HTMLElement).closest('.no-expand')) return;
                    setIsExpanded(!isExpanded);
                }}
            >
                <div className="flex items-center gap-3">
                    <div className="text-gray-500 dark:text-gray-400">
                        {icon}
                    </div>
                    <span className="font-medium text-sm text-gray-900 dark:text-gray-100">
                        {title}
                    </span>
                    {enabled !== undefined && (
                        <div className={`text-xs px-2 py-0.5 rounded-full ${enabled ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400' : 'bg-gray-100 text-gray-500 dark:bg-gray-700 dark:text-gray-400'}`}>
                            {enabled ? t('common.enabled') : t('common.disabled')}
                        </div>
                    )}
                </div>

                <div className="flex items-center gap-4 no-expand">
                    {rightElement}

                    {enabled !== undefined && onToggle && (
                        <div className="flex items-center" onClick={(e) => e.stopPropagation()}>
                            <input
                                type="checkbox"
                                className="toggle toggle-sm"
                                checked={enabled}
                                onChange={(e) => onToggle(e.target.checked)}
                            />
                        </div>
                    )}

                    <button
                        className={`p-1 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-700 transition-all duration-200 ${isExpanded ? 'rotate-180' : ''}`}
                    >
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                            <path d="m6 9 6 6 6-6" />
                        </svg>
                    </button>
                </div>
            </div>

            <div
                className={`transition-all duration-300 ease-in-out border-t border-border ${isExpanded ? 'max-h-[2000px] opacity-100' : 'max-h-0 opacity-0 overflow-hidden'
                    }`}
            >
                <div className="p-5 relative">
                    {/* Overlay when disabled */}
                    {enabled === false && (
                        <div className="absolute inset-0 bg-white/60 dark:bg-black/60 z-10 cursor-not-allowed" />
                    )}
                    <div className={enabled === false ? 'opacity-50 pointer-events-none select-none filter blur-[0.5px]' : ''}>
                        {children}
                    </div>
                </div>
            </div>
        </div>
    );
}

export default function ApiProxy() {
    const { t } = useTranslation();
    const navigate = useNavigate();

    const models = [
        // Gemini 3 Series
        {
            id: 'gemini-3-flash',
            name: 'Gemini 3 Flash',
            desc: t('proxy.model.flash_preview'),
            icon: <Zap size={16} />
        },
        {
            id: 'gemini-3-pro-high',
            name: 'Gemini 3 Pro High',
            desc: t('proxy.model.pro_high'),
            icon: <Cpu size={16} />
        },
        {
            id: 'gemini-3-pro-low',
            name: 'Gemini 3 Pro Low',
            desc: t('proxy.model.flash_lite'),
            icon: <Zap size={16} />
        },
        {
            id: 'gemini-3-pro-image',
            name: 'Gemini 3 Pro (Image)',
            desc: t('proxy.model.pro_image_1_1'),
            icon: <ImageIcon size={16} />
        },

        // Gemini 2.5 Series
        {
            id: 'gemini-2.5-flash',
            name: 'Gemini 2.5 Flash',
            desc: t('proxy.model.flash'),
            icon: <Zap size={16} />
        },
        {
            id: 'gemini-2.5-flash-lite',
            name: 'Gemini 2.5 Flash Lite',
            desc: t('proxy.model.flash_lite'),
            icon: <Zap size={16} />
        },
        {
            id: 'gemini-2.5-pro',
            name: 'Gemini 2.5 Pro',
            desc: t('proxy.model.pro_legacy'),
            icon: <Cpu size={16} />
        },
        {
            id: 'gemini-2.5-flash-thinking',
            name: 'Gemini 2.5 Flash (Thinking)',
            desc: t('proxy.model.claude_sonnet_thinking'),
            icon: <BrainCircuit size={16} />
        },

        // Claude Series
        {
            id: 'claude-sonnet-4-5',
            name: 'Claude 4.5 Sonnet',
            desc: t('proxy.model.claude_sonnet'),
            icon: <Sparkles size={16} />
        },
        {
            id: 'claude-sonnet-4-5-thinking',
            name: 'Claude 4.5 Sonnet (Thinking)',
            desc: t('proxy.model.claude_sonnet_thinking'),
            icon: <BrainCircuit size={16} />
        },
        {
            id: 'claude-opus-4-5-thinking',
            name: 'Claude 4.5 Opus (Thinking)',
            desc: t('proxy.model.claude_opus_thinking'),
            icon: <Cpu size={16} />
        }
    ];

    const [status, setStatus] = useState<ProxyStatus>({
        running: false,
        port: 0,
        base_url: '',
        active_accounts: 0,
    });

    const [appConfig, setAppConfig] = useState<AppConfig | null>(null);
    const [loading, setLoading] = useState(false);
    const [copied, setCopied] = useState<string | null>(null);
    const [selectedProtocol, setSelectedProtocol] = useState<'openai' | 'anthropic' | 'gemini'>('openai');
    const [selectedModelId, setSelectedModelId] = useState('gemini-3-flash');
    const [zaiAvailableModels, setZaiAvailableModels] = useState<string[]>([]);
    const [zaiModelsLoading, setZaiModelsLoading] = useState(false);
    const [, setZaiModelsError] = useState<string | null>(null);
    const [zaiNewMappingFrom, setZaiNewMappingFrom] = useState('');
    const [zaiNewMappingTo, setZaiNewMappingTo] = useState('');

    // Minimax state (similar to Z.AI)
    const [minimaxAvailableModels, setMinimaxAvailableModels] = useState<string[]>([]);
    const [minimaxModelsLoading, setMinimaxModelsLoading] = useState(false);
    const [, setMinimaxModelsError] = useState<string | null>(null);
    const [minimaxNewMappingFrom, setMinimaxNewMappingFrom] = useState('');
    const [minimaxNewMappingTo, setMinimaxNewMappingTo] = useState('');

    // Modal states
    const [isResetConfirmOpen, setIsResetConfirmOpen] = useState(false);
    const [isRegenerateKeyConfirmOpen, setIsRegenerateKeyConfirmOpen] = useState(false);
    const [isClearBindingsConfirmOpen, setIsClearBindingsConfirmOpen] = useState(false);

    // Claude integration toggle state
    const [claudeIntegrationEnabled, setClaudeIntegrationEnabled] = useState(false);
    const [claudeIntegrationLoading, setClaudeIntegrationLoading] = useState(false);

    // YOLO mode toggle state
    const [yoloModeEnabled, setYoloModeEnabled] = useState(false);
    const [yoloModeLoading, setYoloModeLoading] = useState(false);

    // OpenCode integration toggle state
    const [opencodeIntegrationEnabled, setOpencodeIntegrationEnabled] = useState(false);
    const [opencodeIntegrationLoading, setOpencodeIntegrationLoading] = useState(false);

    const zaiModelOptions = useMemo(() => {
        const unique = new Set(zaiAvailableModels);
        return Array.from(unique).sort();
    }, [zaiAvailableModels]);

    const zaiModelMapping = useMemo(() => {
        return appConfig?.proxy.zai?.model_mapping || {};
    }, [appConfig?.proxy.zai?.model_mapping]);

    const minimaxModelOptions = useMemo(() => {
        const unique = new Set(minimaxAvailableModels);
        return Array.from(unique).sort();
    }, [minimaxAvailableModels]);

    const minimaxModelMapping = useMemo(() => {
        return appConfig?.proxy.minimax?.model_mapping || {};
    }, [appConfig?.proxy.minimax?.model_mapping]);

    // åˆå§‹åŒ–åŠ è½½
    useEffect(() => {
        loadConfig();
        loadStatus();
        const interval = setInterval(loadStatus, 3000);
        return () => clearInterval(interval);
    }, []);

    // Check Claude integration status when config loads
    useEffect(() => {
        const checkClaudeIntegration = async () => {
            if (!appConfig) return;
            try {
                const enabled = await invoke<boolean>('get_claude_integration_status', {
                    port: appConfig.proxy.port,
                    apiKey: appConfig.proxy.api_key
                });
                setClaudeIntegrationEnabled(enabled);
            } catch (error) {
                console.error('Failed to check Claude integration status:', error);
            }
        };
        checkClaudeIntegration();
    }, [appConfig?.proxy.port, appConfig?.proxy.api_key]);

    // Check YOLO mode status when Claude integration changes
    useEffect(() => {
        const checkYoloMode = async () => {
            if (!claudeIntegrationEnabled) {
                setYoloModeEnabled(false);
                return;
            }
            try {
                const enabled = await invoke<boolean>('get_yolo_mode_status');
                setYoloModeEnabled(enabled);
            } catch (error) {
                console.error('Failed to check YOLO mode status:', error);
            }
        };
        checkYoloMode();
    }, [claudeIntegrationEnabled]);

    // Check OpenCode integration status when config loads
    useEffect(() => {
        const checkOpencodeIntegration = async () => {
            if (!appConfig) return;
            try {
                const enabled = await invoke<boolean>('get_opencode_integration_status', {
                    port: appConfig.proxy.port,
                    apiKey: appConfig.proxy.api_key
                });
                setOpencodeIntegrationEnabled(enabled);
            } catch (error) {
                console.error('Failed to check OpenCode integration status:', error);
            }
        };
        checkOpencodeIntegration();
    }, [appConfig?.proxy.port, appConfig?.proxy.api_key]);

    // Auto-sync env vars when port/apiKey changes AND integration is enabled
    useEffect(() => {
        const syncEnvVars = async () => {
            if (!appConfig || !claudeIntegrationEnabled) return;
            try {
                // Silently update env vars to match current config
                await invoke('enable_claude_integration', {
                    port: appConfig.proxy.port,
                    apiKey: appConfig.proxy.api_key
                });
                console.log('Auto-synced Claude env vars with current config');
            } catch (error) {
                console.error('Failed to auto-sync Claude env vars:', error);
            }
        };
        syncEnvVars();
    }, [appConfig?.proxy.port, appConfig?.proxy.api_key, claudeIntegrationEnabled]);

    const loadConfig = async () => {
        try {
            const config = await invoke<AppConfig>('load_config');
            setAppConfig(config);
        } catch (error) {
            console.error('åŠ è½½é…ç½®å¤±è´¥:', error);
        }
    };

    const loadStatus = async () => {
        try {
            const s = await invoke<ProxyStatus>('get_proxy_status');
            setStatus(s);
        } catch (error) {
            console.error('èŽ·å–çŠ¶æ€å¤±è´¥:', error);
        }
    };

    const saveConfig = async (newConfig: AppConfig) => {
        try {
            await invoke('save_config', { config: newConfig });
            setAppConfig(newConfig);
        } catch (error) {
            console.error('ä¿å­˜é…ç½®å¤±è´¥:', error);
            showToast(`${t('common.error')}: ${error}`, 'error');
        }
    };

    // Handle Claude integration toggle
    const handleClaudeIntegrationToggle = async (enable: boolean) => {
        if (!appConfig) return;
        setClaudeIntegrationLoading(true);
        try {
            if (enable) {
                // Configure env vars for Claude CLI/Code
                await invoke('enable_claude_integration', {
                    port: appConfig.proxy.port,
                    apiKey: appConfig.proxy.api_key
                });
                // Also configure Claude Desktop MCP
                try {
                    await invoke('configure_claude_desktop', {
                        port: appConfig.proxy.port,
                        apiKey: appConfig.proxy.api_key
                    });
                    showToast(t('proxy.client_integration.all_enabled', { defaultValue: 'Claude CLI, Code & Desktop configured! Restart Claude Desktop to apply.' }), 'success');
                } catch (desktopErr) {
                    console.warn('Claude Desktop MCP config failed:', desktopErr);
                    showToast(t('proxy.client_integration.toggle_enable_success'), 'success');
                }
            } else {
                await invoke('disable_claude_integration');
                showToast(t('proxy.client_integration.toggle_disable_success'), 'success');
            }
            setClaudeIntegrationEnabled(enable);
        } catch (error) {
            console.error('Failed to toggle Claude integration:', error);
            showToast(`${t('proxy.client_integration.toggle_error')}: ${error}`, 'error');
        } finally {
            setClaudeIntegrationLoading(false);
        }
    };

    // Handle OpenCode integration toggle
    const handleOpencodeIntegrationToggle = async (enable: boolean) => {
        if (!appConfig) return;
        setOpencodeIntegrationLoading(true);
        try {
            if (enable) {
                const result = await invoke<string>('enable_opencode_integration', {
                    port: appConfig.proxy.port,
                    apiKey: appConfig.proxy.api_key
                });
                showToast('OpenCode integration enabled! Use local.* models in OpenCode.', 'success');
                console.log(result);
            } else {
                await invoke<string>('disable_opencode_integration');
                showToast('OpenCode integration disabled.', 'success');
            }
            setOpencodeIntegrationEnabled(enable);
        } catch (error) {
            console.error('Failed to toggle OpenCode integration:', error);
            showToast(`OpenCode integration error: ${error}`, 'error');
        } finally {
            setOpencodeIntegrationLoading(false);
        }
    };

    // ä¸“é—¨å¤„ç†æ¨¡åž‹æ˜ å°„çš„çƒ­æ›´æ–° (å…¨é‡)
    const handleMappingUpdate = async (type: 'anthropic' | 'openai' | 'custom', key: string, value: string) => {
        if (!appConfig) return;

        const newConfig = { ...appConfig.proxy };
        if (type === 'anthropic') {
            newConfig.anthropic_mapping = { ...(newConfig.anthropic_mapping || {}), [key]: value };
        } else if (type === 'openai') {
            newConfig.openai_mapping = { ...(newConfig.openai_mapping || {}), [key]: value };
        } else if (type === 'custom') {
            newConfig.custom_mapping = { ...(newConfig.custom_mapping || {}), [key]: value };
        }

        try {
            await invoke('update_model_mapping', { config: newConfig });
            setAppConfig({ ...appConfig, proxy: newConfig });
        } catch (error) {
            console.error('Failed to update mapping:', error);
        }
    };

    const handleResetMapping = () => {
        if (!appConfig) return;
        setIsResetConfirmOpen(true);
    };

    const executeResetMapping = async () => {
        if (!appConfig) return;
        setIsResetConfirmOpen(false);

        // æ¢å¤åˆ°é»˜è®¤æ˜ å°„å€¼
        const newConfig = {
            ...appConfig.proxy,
            anthropic_mapping: {
                'claude-4.5-series': 'gemini-3-pro-high',
                'claude-3.5-series': 'claude-sonnet-4-5-thinking'
            },
            openai_mapping: {
                'gpt-4-series': 'gemini-3-pro-high',
                'gpt-4o-series': 'gemini-3-flash',
                'gpt-5-series': 'gemini-3-flash'
            },
            custom_mapping: {}
        };

        try {
            await invoke('update_model_mapping', { config: newConfig });
            setAppConfig({ ...appConfig, proxy: newConfig });
            showToast(t('common.success'), 'success');
        } catch (error) {
            console.error('Failed to reset mapping:', error);
            showToast(`${t('common.error')}: ${error}`, 'error');
        }
    };

    // ä¸€é”®æ·»åŠ  Haiku ä¼˜åŒ–æ˜ å°„
    const handleAddHaikuOptimization = async () => {
        const originalModel = 'claude-haiku-4-5-20251001';
        const targetModel = 'gemini-2.5-flash-lite';

        // è°ƒç”¨çŽ°æœ‰çš„ handleMappingUpdate å‡½æ•°
        await handleMappingUpdate('custom', originalModel, targetModel);

        // æ»šåŠ¨åˆ°è‡ªå®šä¹‰æ˜ å°„åˆ—è¡¨ (å¯é€‰,æå‡ UX)
        setTimeout(() => {
            const customListElement = document.querySelector('[data-custom-mapping-list]');
            if (customListElement) {
                customListElement.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
            }
        }, 100);
    };

    const handleRemoveCustomMapping = async (key: string) => {
        if (!appConfig || !appConfig.proxy.custom_mapping) return;
        const newCustom = { ...appConfig.proxy.custom_mapping };
        delete newCustom[key];
        const newConfig = { ...appConfig.proxy, custom_mapping: newCustom };
        try {
            await invoke('update_model_mapping', { config: newConfig });
            setAppConfig({ ...appConfig, proxy: newConfig });
        } catch (error) {
            console.error('Failed to remove custom mapping:', error);
        }
    };

    const updateProxyConfig = async (updates: Partial<ProxyConfig>) => {
        if (!appConfig) return;

        const portChanged = updates.port !== undefined && updates.port !== appConfig.proxy.port;
        const baseUrlChanged = updates.base_url_override !== undefined && updates.base_url_override !== appConfig.proxy.base_url_override;
        const apiKeyChanged = updates.api_key !== undefined && updates.api_key !== appConfig.proxy.api_key;

        const newConfig = {
            ...appConfig,
            proxy: {
                ...appConfig.proxy,
                ...updates
            }
        };

        // Save the config
        await saveConfig(newConfig);

        // Auto-restart service if port changed while running (if enabled)
        if (portChanged && status.running) {
            const autoRestartEnabled = newConfig.proxy.auto_restart_on_config_change ?? true;

            if (autoRestartEnabled) {
                try {
                    showToast(t('proxy.config.restarting_service') || 'Restarting service with new port...', 'info');
                    await invoke('stop_proxy_service');
                    await invoke('start_proxy_service', { config: newConfig.proxy });
                    await loadStatus();
                    showToast(t('proxy.config.restart_success') || 'Service restarted with new configuration', 'success');
                } catch (error) {
                    console.error('Failed to restart service:', error);
                    showToast(`${t('proxy.config.restart_failed') || 'Failed to restart service'}: ${error}`, 'error');
                }
            } else {
                // Show message that manual restart is required
                showToast(t('proxy.config.manual_restart_required') || 'Port changed. Manual restart required to apply new configuration.', 'warning');
            }
        }

        // Auto-sync Claude Desktop config if Client Integration is enabled and config changed
        if (claudeIntegrationEnabled && (portChanged || baseUrlChanged || apiKeyChanged)) {
            try {
                const effectivePort = newConfig.proxy.port;
                // Update env vars for CLI/Code
                await invoke('enable_claude_integration', {
                    port: effectivePort,
                    apiKey: newConfig.proxy.api_key
                });
                // Also update Claude Desktop MCP config
                try {
                    await invoke('configure_claude_desktop', {
                        port: effectivePort,
                        apiKey: newConfig.proxy.api_key
                    });
                } catch (desktopErr) {
                    console.warn('Claude Desktop MCP update skipped:', desktopErr);
                }
                showToast(t('proxy.config.claude_config_synced') || 'Claude Desktop config updated with new settings', 'success');
            } catch (error) {
                console.error('Failed to sync Claude config:', error);
                showToast(`${t('proxy.config.claude_sync_failed') || 'Failed to update Claude config'}: ${error}`, 'warning');
            }
        }
    };

    const updateSchedulingConfig = (updates: Partial<StickySessionConfig>) => {
        if (!appConfig) return;
        const currentScheduling = appConfig.proxy.scheduling || { mode: 'Balance', max_wait_seconds: 60 };
        const newScheduling = { ...currentScheduling, ...updates };

        const newAppConfig = {
            ...appConfig,
            proxy: {
                ...appConfig.proxy,
                scheduling: newScheduling
            }
        };
        saveConfig(newAppConfig);
    };

    const handleClearSessionBindings = () => {
        setIsClearBindingsConfirmOpen(true);
    };

    const executeClearSessionBindings = async () => {
        setIsClearBindingsConfirmOpen(false);
        try {
            await invoke('clear_proxy_session_bindings');
            showToast(t('common.success'), 'success');
        } catch (error) {
            console.error('Failed to clear session bindings:', error);
            showToast(`${t('common.error')}: ${error}`, 'error');
        }
    };

    const refreshZaiModels = async () => {
        if (!appConfig?.proxy.zai) return;
        setZaiModelsLoading(true);
        setZaiModelsError(null);
        try {
            const models = await invoke<string[]>('fetch_zai_models', {
                zai: appConfig.proxy.zai,
                upstreamProxy: appConfig.proxy.upstream_proxy,
                requestTimeout: appConfig.proxy.request_timeout,
            });
            setZaiAvailableModels(models);
        } catch (error: any) {
            console.error('Failed to fetch z.ai models:', error);
            setZaiModelsError(error.toString());
        } finally {
            setZaiModelsLoading(false);
        }
    };

    const updateZaiDefaultModels = (updates: Partial<NonNullable<ProxyConfig['zai']>['models']>) => {
        if (!appConfig?.proxy.zai) return;
        const newConfig = {
            ...appConfig,
            proxy: {
                ...appConfig.proxy,
                zai: {
                    ...appConfig.proxy.zai,
                    models: { ...appConfig.proxy.zai.models, ...updates }
                }
            }
        };
        saveConfig(newConfig);
    };

    const upsertZaiModelMapping = (from: string, to: string) => {
        if (!appConfig?.proxy.zai) return;
        const currentMapping = appConfig.proxy.zai.model_mapping || {};
        const newMapping = { ...currentMapping, [from]: to };

        const newConfig = {
            ...appConfig,
            proxy: {
                ...appConfig.proxy,
                zai: {
                    ...appConfig.proxy.zai,
                    model_mapping: newMapping
                }
            }
        };
        saveConfig(newConfig);
    };

    const removeZaiModelMapping = (from: string) => {
        if (!appConfig?.proxy.zai) return;
        const currentMapping = appConfig.proxy.zai.model_mapping || {};
        const newMapping = { ...currentMapping };
        delete newMapping[from];

        const newConfig = {
            ...appConfig,
            proxy: {
                ...appConfig.proxy,
                zai: {
                    ...appConfig.proxy.zai,
                    model_mapping: newMapping
                }
            }
        };
        saveConfig(newConfig);
    };

    const updateZaiGeneralConfig = (updates: Partial<NonNullable<ProxyConfig['zai']>>) => {
        if (!appConfig?.proxy.zai) return;
        const newConfig = {
            ...appConfig,
            proxy: {
                ...appConfig.proxy,
                zai: {
                    ...appConfig.proxy.zai,
                    ...updates
                }
            }
        };
        saveConfig(newConfig);

        // Show toast notification for specific changes
        if ('api_key' in updates && updates.api_key) {
            showToast(t('proxy.config.zai.api_key_saved', { defaultValue: 'Z.AI API Key saved' }), 'success');
        } else if ('enabled' in updates) {
            showToast(updates.enabled
                ? t('proxy.config.zai.enabled_toast', { defaultValue: 'Z.AI enabled' })
                : t('proxy.config.zai.disabled_toast', { defaultValue: 'Z.AI disabled' }), 'info');
        }
    };

    // ======== Minimax Helper Functions ========

    const refreshMinimaxModels = async () => {
        if (!appConfig?.proxy.minimax) return;
        setMinimaxModelsLoading(true);
        setMinimaxModelsError(null);
        try {
            const models = await invoke<string[]>('fetch_minimax_models', {
                minimax: appConfig.proxy.minimax,
                upstreamProxy: appConfig.proxy.upstream_proxy,
                requestTimeout: appConfig.proxy.request_timeout,
            });
            setMinimaxAvailableModels(models);
        } catch (error: any) {
            console.error('Failed to fetch Minimax models:', error);
            setMinimaxModelsError(error.toString());
            showToast(`Failed to fetch Minimax models: ${error}`, 'error');
        } finally {
            setMinimaxModelsLoading(false);
        }
    };

    const updateMinimaxDefaultModels = (updates: Partial<NonNullable<ProxyConfig['minimax']>['models']>) => {
        if (!appConfig?.proxy.minimax) return;
        const newConfig = {
            ...appConfig,
            proxy: {
                ...appConfig.proxy,
                minimax: {
                    ...appConfig.proxy.minimax,
                    models: {
                        ...appConfig.proxy.minimax.models,
                        ...updates
                    }
                }
            }
        };
        saveConfig(newConfig);
    };

    const upsertMinimaxModelMapping = (from: string, to: string) => {
        if (!appConfig?.proxy.minimax) return;
        const newMapping = {
            ...(appConfig.proxy.minimax.model_mapping || {}),
            [from]: to
        };
        const newConfig = {
            ...appConfig,
            proxy: {
                ...appConfig.proxy,
                minimax: {
                    ...appConfig.proxy.minimax,
                    model_mapping: newMapping
                }
            }
        };
        saveConfig(newConfig);
    };

    const removeMinimaxModelMapping = (from: string) => {
        if (!appConfig?.proxy.minimax?.model_mapping) return;
        const newMapping = { ...appConfig.proxy.minimax.model_mapping };
        delete newMapping[from];
        const newConfig = {
            ...appConfig,
            proxy: {
                ...appConfig.proxy,
                minimax: {
                    ...appConfig.proxy.minimax,
                    model_mapping: newMapping
                }
            }
        };
        saveConfig(newConfig);
    };

    const updateMinimaxGeneralConfig = (updates: Partial<NonNullable<ProxyConfig['minimax']>>) => {
        if (!appConfig) return;

        // Initialize with defaults if minimax doesn't exist
        const currentMinimax = appConfig.proxy.minimax || {
            enabled: false,
            base_url: 'https://api.minimax.io/v1',
            api_key: '',
            dispatch_mode: 'off' as const,
            model_mapping: {},
            models: {
                opus: 'MiniMax-Text-01',
                sonnet: 'MiniMax-Text-01',
                haiku: 'abab6.5s-chat'
            }
        };

        const newConfig = {
            ...appConfig,
            proxy: {
                ...appConfig.proxy,
                minimax: {
                    ...currentMinimax,
                    ...updates
                }
            }
        };
        saveConfig(newConfig);

        // Show toast notification for specific changes
        if ('api_key' in updates && updates.api_key) {
            showToast(t('proxy.config.minimax.api_key_saved', { defaultValue: 'Minimax API Key saved' }), 'success');
        } else if ('enabled' in updates) {
            showToast(updates.enabled
                ? t('proxy.config.minimax.enabled_toast', { defaultValue: 'Minimax enabled' })
                : t('proxy.config.minimax.disabled_toast', { defaultValue: 'Minimax disabled' }), 'info');
        }
    };

    const handleToggle = async () => {
        if (!appConfig) return;
        setLoading(true);
        try {
            if (status.running) {
                await invoke('stop_proxy_service');
            } else {
                // Start the proxy service
                await invoke('start_proxy_service', { config: appConfig.proxy });

                // ALWAYS configure Claude integration when service starts for seamless workflow
                // This ensures any new terminal will be able to use Claude immediately
                try {
                    // Configure environment variables for Claude CLI/Code
                    await invoke('enable_claude_integration', {
                        port: appConfig.proxy.port,
                        apiKey: appConfig.proxy.api_key
                    });

                    // Enable the toggle since we just configured it
                    if (!claudeIntegrationEnabled) {
                        setClaudeIntegrationEnabled(true);
                    }

                    // Auto-refresh environment so new terminals work immediately
                    try {
                        await invoke('refresh_environment');
                        showToast('Service started! Claude CLI ready in any new terminal.', 'success');
                    } catch (refreshError) {
                        console.warn('Environment refresh failed:', refreshError);
                        showToast('Service started! Open a new terminal to use Claude CLI.', 'success');
                    }

                    // Also configure Claude Desktop MCP server  
                    try {
                        await invoke('configure_claude_desktop', {
                            port: appConfig.proxy.port,
                            apiKey: appConfig.proxy.api_key
                        });
                    } catch (desktopError) {
                        console.warn('Claude Desktop MCP config failed (may need Python):', desktopError);
                    }

                    // Auto-configure OpenCode integration
                    try {
                        await invoke<string>('enable_opencode_integration', {
                            port: appConfig.proxy.port,
                            apiKey: appConfig.proxy.api_key
                        });
                        if (!opencodeIntegrationEnabled) {
                            setOpencodeIntegrationEnabled(true);
                        }
                    } catch (opencodeError) {
                        console.warn('OpenCode integration auto-config failed:', opencodeError);
                    }
                } catch (integrationError) {
                    console.error('Auto-configure Claude failed:', integrationError);
                    showToast('Service started. Manual config may be needed for Claude CLI.', 'warning');
                }
            }
            await loadStatus();
        } catch (error: any) {
            const errorStr = error.toString();
            // Detect port-in-use errors (Windows: 10048, Linux/Mac: EADDRINUSE)
            if (errorStr.includes('10048') || errorStr.includes('EADDRINUSE') || errorStr.includes('address already in use')) {
                showToast(
                    t('proxy.error.port_in_use', {
                        defaultValue: `Port ${appConfig.proxy.port} is already in use. Please change to a different port (e.g., 18046, 28045) and try again.`,
                        port: appConfig.proxy.port
                    }),
                    'error'
                );
            } else {
                showToast(t('proxy.dialog.operate_failed', { error: errorStr }), 'error');
            }
        } finally {
            setLoading(false);
        }
    };

    const handleGenerateApiKey = () => {
        setIsRegenerateKeyConfirmOpen(true);
    };

    const executeGenerateApiKey = async () => {
        setIsRegenerateKeyConfirmOpen(false);
        try {
            const newKey = await invoke<string>('generate_api_key');
            updateProxyConfig({ api_key: newKey });
            showToast(t('common.success'), 'success');
        } catch (error: any) {
            console.error('ç”Ÿæˆ API Key å¤±è´¥:', error);
            showToast(t('proxy.dialog.operate_failed', { error: error.toString() }), 'error');
        }
    };

    const copyToClipboard = (text: string, label: string) => {
        navigator.clipboard.writeText(text).then(() => {
            setCopied(label);
            setTimeout(() => setCopied(null), 2000);
        });
    };


    const getPythonExample = (modelId: string) => {
        const port = status.running ? status.port : (appConfig?.proxy.port || 9000);
        // æŽ¨èä½¿ç”¨ 127.0.0.1 ä»¥é¿å…éƒ¨åˆ†çŽ¯å¢ƒ IPv6 è§£æžå»¶è¿Ÿé—®é¢˜
        const baseUrl = `http://127.0.0.1:${port}/v1`;
        const apiKey = appConfig?.proxy.api_key || 'YOUR_API_KEY';

        // 1. Anthropic Protocol
        if (selectedProtocol === 'anthropic') {
            return `from anthropic import Anthropic
 
 client = Anthropic(
     # æŽ¨èä½¿ç”¨ 127.0.0.1
     base_url="${`http://127.0.0.1:${port}`}",
     api_key="${apiKey}"
 )
 
 # æ³¨æ„: Antigravity æ”¯æŒä½¿ç”¨ Anthropic SDK è°ƒç”¨ä»»æ„æ¨¡åž‹
 response = client.messages.create(
     model="${modelId}",
     max_tokens=1024,
     messages=[{"role": "user", "content": "Hello"}]
 )
 
 print(response.content[0].text)`;
        }

        // 2. Gemini Protocol (Native)
        if (selectedProtocol === 'gemini') {
            const rawBaseUrl = `http://127.0.0.1:${port}`;
            return `# éœ€è¦å®‰è£…: pip install google-generativeai
import google.generativeai as genai

# ä½¿ç”¨ Antigravity ä»£ç†åœ°å€ (æŽ¨è 127.0.0.1)
genai.configure(
    api_key="${apiKey}",
    transport='rest',
    client_options={'api_endpoint': '${rawBaseUrl}'}
)

model = genai.GenerativeModel('${modelId}')
response = model.generate_content("Hello")
print(response.text)`;
        }

        // 3. OpenAI Protocol
        if (modelId.startsWith('gemini-3-pro-image')) {
            return `from openai import OpenAI
 
 client = OpenAI(
     base_url="${baseUrl}",
     api_key="${apiKey}"
 )
 
 response = client.chat.completions.create(
     model="${modelId}",
     # æ–¹å¼ 1: ä½¿ç”¨ size å‚æ•° (æŽ¨è)
     # æ”¯æŒ: "1024x1024" (1:1), "1280x720" (16:9), "720x1280" (9:16), "1216x896" (4:3)
     extra_body={ "size": "1024x1024" },
     
     # æ–¹å¼ 2: ä½¿ç”¨æ¨¡åž‹åŽç¼€
     # ä¾‹å¦‚: gemini-3-pro-image-16-9, gemini-3-pro-image-4-3
     # model="gemini-3-pro-image-16-9",
     messages=[{
         "role": "user",
         "content": "Draw a futuristic city"
     }]
 )
 
 print(response.choices[0].message.content)`;
        }

        return `from openai import OpenAI
 
 client = OpenAI(
     base_url="${baseUrl}",
     api_key="${apiKey}"
 )
 
 response = client.chat.completions.create(
     model="${modelId}",
     messages=[{"role": "user", "content": "Hello"}]
 )
 
 print(response.choices[0].message.content)`;
    };

    // åœ¨ filter é€»è¾‘ä¸­ï¼Œå½“é€‰æ‹© openai åè®®æ—¶ï¼Œå…è®¸æ˜¾ç¤ºæ‰€æœ‰æ¨¡åž‹
    const filteredModels = models.filter(model => {
        if (selectedProtocol === 'openai') {
            return true;
        }
        // Anthropic åè®®ä¸‹éšè—ä¸æ”¯æŒçš„å›¾ç‰‡æ¨¡åž‹
        if (selectedProtocol === 'anthropic') {
            return !model.id.includes('image');
        }
        return true;
    });

    return (
        <div className="h-full w-full overflow-y-auto overflow-x-hidden">
            <div className="p-5 space-y-4 max-w-7xl mx-auto">


                {/* é…ç½®åŒº */}
                {appConfig && (
                    <div className="bg-card rounded-xl shadow-sm border border-border">
                        <div className="px-4 py-2.5 border-b border-border flex items-center justify-between">
                            <div className="flex items-center gap-4">
                                <h2 className="text-base font-semibold flex items-center gap-2 text-card-foreground">
                                    <Settings size={18} />
                                    {t('proxy.config.title')}
                                </h2>
                                {/* çŠ¶æ€æŒ‡ç¤ºå™¨ */}
                                <div className="flex items-center gap-2 pl-4 border-l border-border">
                                    <div className={`w-2 h-2 rounded-full ${status.running ? 'bg-green-500 animate-pulse' : 'bg-gray-400'}`} />
                                    <span className={`text-xs font-medium ${status.running ? 'text-green-600' : 'text-gray-500'}`}>
                                        {status.running
                                            ? `${t('proxy.status.running')} (${status.active_accounts} ${t('common.accounts') || 'Accounts'})`
                                            : t('proxy.status.stopped')}
                                    </span>
                                </div>
                            </div>

                            {/* Control buttons */}
                            <div className="flex items-center gap-2">
                                {status.running && (
                                    <button
                                        onClick={() => navigate('/monitor')}
                                        className="px-3 py-1 rounded-lg text-xs font-medium transition-colors flex items-center gap-2 border bg-white dark:bg-white/90 text-gray-700 dark:text-gray-800 border-gray-200 dark:border-gray-300 hover:bg-gray-100 dark:hover:bg-white hover:text-black dark:hover:text-black"
                                    >
                                        <Activity size={14} />
                                        {t('monitor.open_monitor')}
                                    </button>
                                )}
                                <button
                                    onClick={handleToggle}
                                    disabled={loading || !appConfig}
                                    className={`px-3 py-1 rounded-lg text-xs font-medium transition-colors flex items-center gap-2 ${status.running
                                        ? 'bg-red-50 to-red-600 text-red-600 hover:bg-red-100 border border-red-200'
                                        : 'bg-blue-600 hover:bg-blue-700 text-white shadow-sm shadow-blue-500/30'
                                        } ${(loading || !appConfig) ? 'opacity-50 cursor-not-allowed' : ''}`}
                                >
                                    <Power size={14} />
                                    {loading ? t('proxy.status.processing') : (status.running ? t('proxy.action.stop') : t('proxy.action.start'))}
                                </button>
                            </div>
                        </div>
                        <div className="p-3 space-y-3">
                            {/* Auto Start toggles row */}
                            <div className="flex items-center gap-6">
                                <div className="flex items-center">
                                    <label className="flex items-center cursor-pointer gap-3">
                                        <input
                                            type="checkbox"
                                            className="toggle toggle-sm"
                                            checked={appConfig.proxy.auto_start}
                                            onChange={(e) => updateProxyConfig({ auto_start: e.target.checked })}
                                        />
                                        <span className="text-xs font-medium text-card-foreground inline-flex items-center gap-1">
                                            {t('proxy.config.auto_start')}
                                            <HelpTooltip
                                                text={t('proxy.config.auto_start_tooltip')}
                                                ariaLabel={t('proxy.config.auto_start')}
                                                placement="right"
                                            />
                                        </span>
                                    </label>
                                </div>
                                <div className="flex items-center">
                                    <label className="flex items-center cursor-pointer gap-3">
                                        <input
                                            type="checkbox"
                                            className="toggle toggle-sm"
                                            checked={appConfig.proxy.auto_restart_on_config_change ?? true}
                                            onChange={(e) => updateProxyConfig({ auto_restart_on_config_change: e.target.checked })}
                                        />
                                        <span className="text-xs font-medium text-card-foreground inline-flex items-center gap-1">
                                            {t('proxy.config.auto_restart') || 'Auto-Restart'}
                                            <HelpTooltip
                                                text={t('proxy.config.auto_restart_tooltip') || 'When ON: Automatically restarts the proxy service when port or Base URL changes while running. Also updates Claude Desktop, CLI, and Extension configs with new settings so they work seamlessly without manual restart.'}
                                                ariaLabel={t('proxy.config.auto_restart') || 'Auto-Restart'}
                                                placement="right"
                                            />
                                        </span>
                                    </label>
                                </div>
                                <div className="flex items-center">
                                    <label className="flex items-center cursor-pointer gap-3">
                                        <input
                                            type="checkbox"
                                            className="toggle toggle-sm toggle-success"
                                            checked={opencodeIntegrationEnabled}
                                            disabled={opencodeIntegrationLoading}
                                            onChange={(e) => handleOpencodeIntegrationToggle(e.target.checked)}
                                        />
                                        <span className="text-xs font-medium text-card-foreground inline-flex items-center gap-1">
                                            OpenCode
                                            <HelpTooltip
                                                text="Enable OpenCode CLI/Desktop integration. Sets LOCAL_ENDPOINT and creates opencode.json config for seamless use of Antigravity's combined quota in OpenCode."
                                                ariaLabel="OpenCode Integration"
                                                placement="right"
                                            />
                                        </span>
                                    </label>
                                </div>
                            </div>

                            {/* API Key */}
                            <div>
                                <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    <span className="inline-flex items-center gap-1">
                                        {t('proxy.config.api_key')}
                                        <HelpTooltip
                                            text={t('proxy.config.api_key_tooltip')}
                                            ariaLabel={t('proxy.config.api_key')}
                                            placement="right"
                                        />
                                    </span>
                                </label>
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        value={appConfig.proxy.api_key}
                                        readOnly
                                        className="flex-1 px-2.5 py-1.5 border border-border rounded-lg bg-gray-50 dark:bg-base-300 text-xs text-gray-600 dark:text-gray-400 font-mono"
                                    />
                                    <button
                                        onClick={handleGenerateApiKey}
                                        className="px-2.5 py-1.5 border border-border rounded-lg bg-muted hover:bg-gray-50 dark:hover:bg-base-300 transition-colors"
                                        title={t('proxy.config.btn_regenerate')}
                                    >
                                        <RefreshCw size={14} />
                                    </button>
                                    <button
                                        onClick={() => copyToClipboard(appConfig.proxy.api_key, 'api_key')}
                                        className="px-2.5 py-1.5 border border-border rounded-lg bg-muted hover:bg-gray-50 dark:hover:bg-base-300 transition-colors"
                                        title={t('proxy.config.btn_copy')}
                                    >
                                        {copied === 'api_key' ? (
                                            <CheckCircle size={14} className="text-green-500" />
                                        ) : (
                                            <Copy size={14} />
                                        )}
                                    </button>
                                </div>
                                <p className="mt-0.5 text-[10px] text-amber-600 dark:text-amber-500">
                                    {t('proxy.config.warning_key')}
                                </p>
                            </div>

                            {/* Base URL */}
                            <div>
                                <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                    <span className="inline-flex items-center gap-1">
                                        {t('proxy.client_integration.base_url')}
                                        <HelpTooltip
                                            text={t('proxy.config.base_url_tooltip') || 'Base URL for API clients. Auto-generated from port, or enter custom URL for port conflicts.'}
                                            ariaLabel={t('proxy.client_integration.base_url')}
                                            placement="right"
                                        />
                                    </span>
                                </label>
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        value={appConfig.proxy.base_url_override || `http://127.0.0.1:${appConfig.proxy.port}`}
                                        onChange={(e) => {
                                            const url = e.target.value;
                                            try {
                                                const urlObj = new URL(url);
                                                const newPort = parseInt(urlObj.port) || (urlObj.protocol === 'https:' ? 443 : 80);
                                                if (newPort >= 1 && newPort <= 65535) {
                                                    updateProxyConfig({ base_url_override: url, port: newPort });
                                                } else {
                                                    updateProxyConfig({ base_url_override: url });
                                                }
                                            } catch {
                                                updateProxyConfig({ base_url_override: url });
                                            }
                                        }}
                                        placeholder={`http://127.0.0.1:${appConfig.proxy.port}`}
                                        className="flex-1 px-2.5 py-1.5 border border-border rounded-lg bg-white dark:bg-base-300 text-xs text-gray-700 dark:text-gray-300 font-mono focus:ring-2 focus:ring-blue-500"
                                    />
                                    <button
                                        onClick={() => updateProxyConfig({ base_url_override: undefined })}
                                        className="px-2.5 py-1.5 border border-border rounded-lg bg-muted hover:bg-gray-50 dark:hover:bg-base-300 transition-colors"
                                        title={t('proxy.config.btn_reset') || 'Reset to default'}
                                    >
                                        <RefreshCw size={14} />
                                    </button>
                                    <button
                                        onClick={() => copyToClipboard(appConfig.proxy.base_url_override || `http://127.0.0.1:${appConfig.proxy.port}`, 'base_url')}
                                        className="px-2.5 py-1.5 border border-border rounded-lg bg-muted hover:bg-gray-50 dark:hover:bg-base-300 transition-colors"
                                        title={t('proxy.config.btn_copy')}
                                    >
                                        {copied === 'base_url' ? (
                                            <CheckCircle size={14} className="text-green-500" />
                                        ) : (
                                            <Copy size={14} />
                                        )}
                                    </button>
                                </div>
                                <p className="mt-0.5 text-[10px] text-gray-500 dark:text-gray-400">
                                    {t('proxy.config.base_url_hint') || 'Enter custom URL or click reset to use default from port. Used for Claude Desktop, CLI & Extension.'}
                                </p>
                            </div>

                            {/* Listen Port & Request Timeout */}
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                                <div>
                                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        <span className="inline-flex items-center gap-1">
                                            {t('proxy.config.port')}
                                            <HelpTooltip
                                                text={t('proxy.config.port_tooltip')}
                                                ariaLabel={t('proxy.config.port')}
                                                placement="right"
                                            />
                                        </span>
                                    </label>
                                    <input
                                        type="number"
                                        value={appConfig.proxy.port}
                                        onChange={(e) => {
                                            const newPort = parseInt(e.target.value);
                                            // Clear base_url_override when port changes so Base URL auto-generates
                                            updateProxyConfig({ port: newPort, base_url_override: undefined });
                                        }}
                                        min={8000}
                                        max={65535}
                                        disabled={status.running}
                                        className="w-full px-2.5 py-1.5 border border-border rounded-lg bg-muted text-xs text-card-foreground focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:opacity-50 disabled:cursor-not-allowed"
                                    />
                                    <p className="mt-0.5 text-[10px] text-gray-500 dark:text-gray-400">
                                        {t('proxy.config.port_hint')}
                                    </p>
                                </div>
                                <div>
                                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                                        <span className="inline-flex items-center gap-1">
                                            {t('proxy.config.request_timeout')}
                                            <HelpTooltip
                                                text={t('proxy.config.request_timeout_tooltip')}
                                                ariaLabel={t('proxy.config.request_timeout')}
                                                placement="top"
                                            />
                                        </span>
                                    </label>
                                    <input
                                        type="number"
                                        value={appConfig.proxy.request_timeout || 120}
                                        onChange={(e) => {
                                            const value = parseInt(e.target.value);
                                            const timeout = Math.max(30, Math.min(600, value));
                                            updateProxyConfig({ request_timeout: timeout });
                                        }}
                                        min={30}
                                        max={600}
                                        disabled={status.running}
                                        className="w-full px-2.5 py-1.5 border border-border rounded-lg bg-muted text-xs text-card-foreground focus:ring-2 focus:ring-blue-500 focus:border-transparent disabled:opacity-50 disabled:cursor-not-allowed"
                                    />
                                    <p className="mt-0.5 text-[10px] text-gray-500 dark:text-gray-400">
                                        {t('proxy.config.request_timeout_hint')}
                                    </p>
                                </div>
                            </div>


                            {/* å±€åŸŸç½‘è®¿é—® & è®¿é—®æŽˆæƒ - åˆå¹¶åˆ°åŒä¸€è¡Œ */}
                            <div className="border-t border-border pt-3 mt-3">
                                <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
                                    {/* å…è®¸å±€åŸŸç½‘è®¿é—® */}
                                    <div className="space-y-2">
                                        <div className="flex items-center justify-between">
                                            <span className="text-xs font-medium text-gray-700 dark:text-gray-300 inline-flex items-center gap-1">
                                                {t('proxy.config.allow_lan_access')}
                                                <HelpTooltip
                                                    text={t('proxy.config.allow_lan_access_tooltip')}
                                                    ariaLabel={t('proxy.config.allow_lan_access')}
                                                    placement="right"
                                                />
                                            </span>
                                            <input
                                                type="checkbox"
                                                className="toggle toggle-sm"
                                                checked={appConfig.proxy.allow_lan_access || false}
                                                onChange={(e) => updateProxyConfig({ allow_lan_access: e.target.checked })}
                                            />
                                        </div>
                                        <p className="text-[10px] text-gray-500 dark:text-gray-400">
                                            {(appConfig.proxy.allow_lan_access || false)
                                                ? t('proxy.config.allow_lan_access_hint_enabled')
                                                : t('proxy.config.allow_lan_access_hint_disabled')}
                                        </p>
                                        {(appConfig.proxy.allow_lan_access || false) && (
                                            <p className="text-[10px] text-amber-600 dark:text-amber-500">
                                                {t('proxy.config.allow_lan_access_warning')}
                                            </p>
                                        )}
                                        {status.running && (
                                            <p className="text-[10px] text-blue-600 dark:text-blue-400">
                                                {t('proxy.config.allow_lan_access_restart_hint')}
                                            </p>
                                        )}
                                    </div>

                                    {/* è®¿é—®æŽˆæƒ */}
                                    <div className="space-y-2">
                                        <div className="flex items-center justify-between">
                                            <label className="text-xs font-medium text-gray-700 dark:text-gray-300">
                                                <span className="inline-flex items-center gap-1">
                                                    {t('proxy.config.auth.title')}
                                                    <HelpTooltip
                                                        text={t('proxy.config.auth.title_tooltip')}
                                                        ariaLabel={t('proxy.config.auth.title')}
                                                        placement="top"
                                                    />
                                                </span>
                                            </label>
                                            <label className="flex items-center cursor-pointer gap-2">
                                                <span className="text-[11px] text-gray-600 dark:text-gray-400 inline-flex items-center gap-1">
                                                    {t('proxy.config.auth.enabled')}
                                                    <HelpTooltip
                                                        text={t('proxy.config.auth.enabled_tooltip')}
                                                        ariaLabel={t('proxy.config.auth.enabled')}
                                                        placement="left"
                                                    />
                                                </span>
                                                <input
                                                    type="checkbox"
                                                    className="toggle toggle-sm"
                                                    checked={(appConfig.proxy.auth_mode || 'off') !== 'off'}
                                                    onChange={(e) => {
                                                        const nextMode = e.target.checked ? 'all_except_health' : 'off';
                                                        updateProxyConfig({ auth_mode: nextMode });
                                                    }}
                                                />
                                            </label>
                                        </div>

                                        <div>
                                            <label className="block text-[11px] text-gray-600 dark:text-gray-400 mb-1">
                                                <span className="inline-flex items-center gap-1">
                                                    {t('proxy.config.auth.mode')}
                                                    <HelpTooltip
                                                        text={t('proxy.config.auth.mode_tooltip')}
                                                        ariaLabel={t('proxy.config.auth.mode')}
                                                        placement="top"
                                                    />
                                                </span>
                                            </label>
                                            <select
                                                value={appConfig.proxy.auth_mode || 'off'}
                                                onChange={(e) =>
                                                    updateProxyConfig({
                                                        auth_mode: e.target.value as ProxyConfig['auth_mode'],
                                                    })
                                                }
                                                className="w-full px-2.5 py-1.5 border border-border rounded-lg bg-muted text-xs text-card-foreground focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                                            >
                                                <option value="off">{t('proxy.config.auth.modes.off')}</option>
                                                <option value="strict">{t('proxy.config.auth.modes.strict')}</option>
                                                <option value="all_except_health">{t('proxy.config.auth.modes.all_except_health')}</option>
                                                <option value="auto">{t('proxy.config.auth.modes.auto')}</option>
                                            </select>
                                            <p className="mt-0.5 text-[10px] text-gray-500 dark:text-gray-400">
                                                {t('proxy.config.auth.hint')}
                                            </p>
                                        </div>
                                    </div>
                                </div>
                            </div>

                        </div>
                    </div>
                )}

                {/* Client Integration Panel */}
                {appConfig && (
                    <div className="bg-card rounded-xl shadow-sm border border-border">
                        <div className="px-4 py-2.5 border-b border-border flex items-center justify-between bg-gray-50/50 dark:bg-gradient-to-b from-[#2a2a2a] to-[#1a1a1a]">
                            <div className="flex items-center gap-3">
                                <h2 className="text-base font-semibold flex items-center gap-2 text-card-foreground">
                                    <Terminal size={18} />
                                    {t('proxy.client_integration.title')}
                                </h2>
                                <div className={`text-xs px-2 py-0.5 rounded-full ${claudeIntegrationEnabled ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400' : 'bg-gray-100 text-gray-500 dark:bg-gray-700 dark:text-gray-400'}`}>
                                    {claudeIntegrationEnabled ? t('proxy.client_integration.toggle_enabled') : t('proxy.client_integration.toggle_disabled')}
                                </div>
                            </div>
                            {/* One-Click Toggle */}
                            <div className="flex items-center gap-3">
                                <span className="text-xs text-gray-500 dark:text-gray-400">
                                    {t('proxy.client_integration.toggle_title')}
                                </span>
                                <input
                                    type="checkbox"
                                    className="toggle toggle-sm toggle-green"
                                    checked={claudeIntegrationEnabled}
                                    disabled={claudeIntegrationLoading}
                                    onChange={(e) => handleClaudeIntegrationToggle(e.target.checked)}
                                />
                            </div>
                        </div>
                        <div className="p-4 space-y-4">
                            <p className="text-xs text-gray-500 dark:text-gray-400">
                                {t('proxy.client_integration.toggle_desc')} {claudeIntegrationEnabled && <span className="text-amber-600">â€¢ {t('proxy.client_integration.requires_restart')}</span>}
                            </p>

                            {/* Refresh Environment Button & YOLO Mode Toggle */}
                            {claudeIntegrationEnabled && (
                                <div className="flex flex-wrap items-center gap-4 pt-2">
                                    <button
                                        onClick={async () => {
                                            try {
                                                await invoke('refresh_environment');
                                                showToast('Environment refreshed! New terminals will now use the proxy.', 'success');
                                            } catch (error) {
                                                console.error('Failed to refresh environment:', error);
                                                showToast(`Failed to refresh: ${error}`, 'error');
                                            }
                                        }}
                                        className="btn btn-sm btn-outline gap-2"
                                    >
                                        <RefreshCw size={14} />
                                        Refresh Environment
                                    </button>

                                    {/* YOLO Mode Toggle */}
                                    <div className="flex items-center gap-3 pl-3 border-l border-border">
                                        <div className="flex items-center gap-2">
                                            <Zap size={16} className="text-amber-500" />
                                            <span className="text-xs font-medium text-gray-700 dark:text-gray-300">YOLO Mode</span>
                                        </div>
                                        <input
                                            type="checkbox"
                                            className="toggle toggle-sm toggle-warning"
                                            checked={yoloModeEnabled}
                                            disabled={yoloModeLoading}
                                            onChange={async (e) => {
                                                setYoloModeLoading(true);
                                                try {
                                                    if (e.target.checked) {
                                                        const port = status.running ? status.port : appConfig.proxy.port;
                                                        const result = await invoke<string>('enable_yolo_mode', {
                                                            port,
                                                            apiKey: appConfig.proxy.api_key
                                                        });
                                                        setYoloModeEnabled(true);
                                                        showToast(result, 'success');
                                                    } else {
                                                        const result = await invoke<string>('disable_yolo_mode');
                                                        setYoloModeEnabled(false);
                                                        showToast(result, 'success');
                                                    }
                                                } catch (error) {
                                                    showToast(`Failed to toggle YOLO mode: ${error}`, 'error');
                                                } finally {
                                                    setYoloModeLoading(false);
                                                }
                                            }}
                                        />
                                    </div>
                                </div>
                            )}

                            {/* YOLO Mode Instructions */}
                            {claudeIntegrationEnabled && yoloModeEnabled && (
                                <div className="bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded-lg p-3 mt-2">
                                    <div className="flex items-start gap-2">
                                        <Zap size={16} className="text-amber-600 dark:text-amber-400 mt-0.5" />
                                        <div className="text-xs">
                                            <p className="font-medium text-amber-800 dark:text-amber-300 mb-1">YOLO Mode Enabled!</p>
                                            <p className="text-amber-700 dark:text-amber-400 mb-2">
                                                Open any <strong>new</strong> PowerShell/Terminal window and type:
                                            </p>
                                            <code className="block bg-amber-100 dark:bg-amber-900/40 px-2 py-1 rounded text-amber-800 dark:text-amber-300 font-mono">
                                                yolo
                                            </code>
                                            <p className="text-amber-600 dark:text-amber-500 mt-2">
                                                This runs <code>claude --dangerously-skip-permissions</code> with your proxy configured automatically.
                                            </p>
                                        </div>
                                    </div>
                                </div>
                            )}

                            {/* Quick Copy Buttons */}
                            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3 pt-2">
                                {/* Claude Code (VS Code Extension) */}
                                <div className="bg-gray-50 dark:bg-base-200 rounded-lg p-3 space-y-2">
                                    <div className="flex items-center gap-2">
                                        <div className="w-8 h-8 bg-gradient-to-br from-orange-500 to-orange-700 rounded-lg flex items-center justify-center">
                                            <Code size={16} className="text-white" />
                                        </div>
                                        <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
                                            Claude Code
                                        </span>
                                    </div>
                                    <button
                                        onClick={() => {
                                            const port = status.running ? status.port : appConfig.proxy.port;
                                            const codeConfig = `// Add to VS Code settings.json:\n{\n  "claude.apiEndpoint": "http://127.0.0.1:${port}",\n  "claude.apiKey": "${appConfig.proxy.api_key}"\n}`;
                                            copyToClipboard(codeConfig, 'code_config');
                                            showToast(t('proxy.client_integration.copied'), 'success');
                                        }}
                                        className="w-full px-3 py-1.5 text-xs font-medium text-orange-600 bg-orange-50 hover:bg-orange-100 dark:bg-orange-900/30 dark:text-orange-400 dark:hover:bg-orange-900/50 rounded-lg transition-colors flex items-center justify-center gap-2"
                                    >
                                        {copied === 'code_config' ? (
                                            <><CheckCircle size={12} /> {t('proxy.client_integration.copied')}</>
                                        ) : (
                                            <><Copy size={12} /> {t('proxy.client_integration.copy_config')}</>
                                        )}
                                    </button>
                                </div>

                                {/* Claude CLI */}
                                <div className="bg-gray-50 dark:bg-base-200 rounded-lg p-3 space-y-2">
                                    <div className="flex items-center gap-2">
                                        <div className="w-8 h-8 bg-gradient-to-br from-green-400 to-green-600 rounded-lg flex items-center justify-center">
                                            <Terminal size={16} className="text-white" />
                                        </div>
                                        <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
                                            {t('proxy.client_integration.claude_cli')}
                                        </span>
                                    </div>
                                    <button
                                        onClick={() => {
                                            const port = status.running ? status.port : appConfig.proxy.port;
                                            const cliCmd = `export ANTHROPIC_BASE_URL="http://127.0.0.1:${port}"\nexport ANTHROPIC_API_KEY="${appConfig.proxy.api_key}"\nclaude chat`;
                                            copyToClipboard(cliCmd, 'cli_cmd');
                                            showToast(t('proxy.client_integration.copied'), 'success');
                                        }}
                                        className="w-full px-3 py-1.5 text-xs font-medium text-green-600 bg-green-50 hover:bg-green-100 dark:bg-green-900/30 dark:text-green-400 dark:hover:bg-green-900/50 rounded-lg transition-colors flex items-center justify-center gap-2"
                                    >
                                        {copied === 'cli_cmd' ? (
                                            <><CheckCircle size={12} /> {t('proxy.client_integration.copied')}</>
                                        ) : (
                                            <><Copy size={12} /> {t('proxy.client_integration.copy_config')}</>
                                        )}
                                    </button>
                                </div>

                                {/* Claude Desktop (MCP) */}
                                <div className="bg-gray-50 dark:bg-base-200 rounded-lg p-3 space-y-2">
                                    <div className="flex items-center gap-2">
                                        <div className="w-8 h-8 bg-gradient-to-br from-amber-400 to-amber-600 rounded-lg flex items-center justify-center">
                                            <Sparkles size={16} className="text-white" />
                                        </div>
                                        <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
                                            Claude Desktop (MCP)
                                        </span>
                                    </div>
                                    <button
                                        onClick={() => {
                                            const port = status.running ? status.port : appConfig.proxy.port;
                                            const envCmd = `$env:ANTHROPIC_BASE_URL = "http://127.0.0.1:${port}"\n$env:ANTHROPIC_API_KEY = "${appConfig.proxy.api_key}"`;
                                            copyToClipboard(envCmd, 'desktop_env');
                                            showToast(t('proxy.client_integration.copied'), 'success');
                                        }}
                                        className="w-full px-3 py-1.5 text-xs font-medium text-amber-600 bg-amber-50 hover:bg-amber-100 dark:bg-amber-900/30 dark:text-amber-400 dark:hover:bg-amber-900/50 rounded-lg transition-colors flex items-center justify-center gap-2"
                                    >
                                        {copied === 'desktop_env' ? (
                                            <><CheckCircle size={12} /> {t('proxy.client_integration.copied')}</>
                                        ) : (
                                            <><Copy size={12} /> {t('proxy.client_integration.copy_env')}</>
                                        )}
                                    </button>
                                </div>

                                {/* Chrome Extension */}
                                <div className="bg-gray-50 dark:bg-base-200 rounded-lg p-3 space-y-2">
                                    <div className="flex items-center gap-2">
                                        <div className="w-8 h-8 bg-gradient-to-br from-purple-400 to-purple-600 rounded-lg flex items-center justify-center">
                                            <Puzzle size={16} className="text-white" />
                                        </div>
                                        <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
                                            {t('proxy.client_integration.chrome_extension')}
                                        </span>
                                    </div>
                                    <button
                                        onClick={() => {
                                            const port = status.running ? status.port : appConfig.proxy.port;
                                            const extConfig = `API URL: http://127.0.0.1:${port}\nAPI Key: ${appConfig.proxy.api_key}`;
                                            copyToClipboard(extConfig, 'ext_config');
                                            showToast(t('proxy.client_integration.copied'), 'success');
                                        }}
                                        className="w-full px-3 py-1.5 text-xs font-medium text-purple-600 bg-purple-50 hover:bg-purple-100 dark:bg-purple-900/30 dark:text-purple-400 dark:hover:bg-purple-900/50 rounded-lg transition-colors flex items-center justify-center gap-2"
                                    >
                                        {copied === 'ext_config' ? (
                                            <><CheckCircle size={12} /> {t('proxy.client_integration.copied')}</>
                                        ) : (
                                            <><Copy size={12} /> {t('proxy.client_integration.copy_config')}</>
                                        )}
                                    </button>
                                </div>
                            </div>

                            {/* PowerShell Permanent Setup */}
                            <div className="border-t border-border pt-3 mt-3">
                                <div className="flex items-center justify-between mb-2">
                                    <span className="text-[11px] font-medium text-gray-500 dark:text-gray-400">
                                        {t('proxy.client_integration.powershell_cmd')} (Permanent)
                                    </span>
                                    <button
                                        onClick={() => {
                                            const port = status.running ? status.port : appConfig.proxy.port;
                                            const permCmd = `[System.Environment]::SetEnvironmentVariable("ANTHROPIC_BASE_URL", "http://127.0.0.1:${port}", "User")\n[System.Environment]::SetEnvironmentVariable("ANTHROPIC_API_KEY", "${appConfig.proxy.api_key}", "User")`;
                                            copyToClipboard(permCmd, 'perm_cmd');
                                            showToast(t('proxy.client_integration.copied'), 'success');
                                        }}
                                        className="px-2.5 py-1 text-[10px] font-medium text-gray-600 bg-gray-100 hover:bg-gray-200 dark:bg-base-300 dark:text-gray-400 dark:hover:bg-base-200 rounded transition-colors flex items-center gap-1"
                                    >
                                        {copied === 'perm_cmd' ? <CheckCircle size={10} /> : <Copy size={10} />}
                                        Copy
                                    </button>
                                </div>
                                <pre className="bg-gray-900 dark:bg-base-300 text-gray-100 dark:text-gray-300 text-[10px] p-2 rounded-lg font-mono overflow-x-auto">
                                    {`[System.Environment]::SetEnvironmentVariable("ANTHROPIC_BASE_URL", "http://127.0.0.1:${status.running ? status.port : appConfig.proxy.port}", "User")
[System.Environment]::SetEnvironmentVariable("ANTHROPIC_API_KEY", "${appConfig.proxy.api_key}", "User")`}
                                </pre>
                                <p className="text-[10px] text-gray-500 dark:text-gray-400 mt-1">
                                    {t('proxy.client_integration.env_hint')}
                                </p>
                            </div>
                        </div>
                    </div>
                )}

                {/* External Providers Integration */}
                {
                    appConfig && (
                        <div className="space-y-4">
                            <div className="px-1 flex items-center gap-2 text-gray-400">
                                <Layers size={14} />
                                <span className="text-[10px] font-bold uppercase tracking-widest">
                                    {t('proxy.config.external_providers.title', { defaultValue: 'External Providers' })}
                                </span>
                            </div>

                            {/* z.ai (GLM) Dispatcher */}
                            <CollapsibleCard
                                title={t('proxy.config.zai.title')}
                                icon={<Zap size={18} className="text-amber-500" />}
                                enabled={!!appConfig.proxy.zai?.enabled}
                                onToggle={(checked) => updateZaiGeneralConfig({ enabled: checked })}
                            >
                                <div className="space-y-4">
                                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                        <div className="space-y-1">
                                            <label className="text-[11px] font-medium text-gray-500 dark:text-gray-400">
                                                {t('proxy.config.zai.base_url')}
                                            </label>
                                            <input
                                                type="text"
                                                value={appConfig.proxy.zai?.base_url || 'https://api.z.ai/api/anthropic'}
                                                onChange={(e) => updateZaiGeneralConfig({ base_url: e.target.value })}
                                                className="input input-sm input-bordered w-full font-mono text-xs"
                                            />
                                        </div>
                                        <div className="space-y-1">
                                            <label className="text-[11px] font-medium text-gray-500 dark:text-gray-400">
                                                {t('proxy.config.zai.dispatch_mode')}
                                            </label>
                                            <select
                                                className="select select-sm select-bordered w-full text-xs"
                                                value={appConfig.proxy.zai?.dispatch_mode || 'off'}
                                                onChange={(e) => updateZaiGeneralConfig({ dispatch_mode: e.target.value as any })}
                                            >
                                                <option value="off">{t('proxy.config.zai.modes.off')}</option>
                                                <option value="exclusive">{t('proxy.config.zai.modes.exclusive')}</option>
                                                <option value="pooled">{t('proxy.config.zai.modes.pooled')}</option>
                                                <option value="fallback">{t('proxy.config.zai.modes.fallback')}</option>
                                            </select>
                                        </div>
                                    </div>

                                    <div className="space-y-1">
                                        <label className="text-[11px] font-medium text-gray-500 dark:text-gray-400 flex items-center justify-between">
                                            <span>{t('proxy.config.zai.api_key')}</span>
                                            {!(appConfig.proxy.zai?.api_key) && (
                                                <span className="text-amber-500 text-[10px] flex items-center gap-1">
                                                    <HelpTooltip text={t('proxy.config.zai.warning')} />
                                                    {t('common.required')}
                                                </span>
                                            )}
                                        </label>
                                        <input
                                            type="password"
                                            value={appConfig.proxy.zai?.api_key || ''}
                                            onChange={(e) => updateZaiGeneralConfig({ api_key: e.target.value })}
                                            placeholder="sk-..."
                                            className="input input-sm input-bordered w-full font-mono text-xs"
                                        />
                                    </div>

                                    {/* Model Mapping Section */}
                                    <div className="pt-4 border-t border-border">
                                        <div className="flex items-center justify-between mb-3">
                                            <h4 className="text-[11px] font-bold text-gray-400 uppercase tracking-widest">
                                                {t('proxy.config.zai.models.title')}
                                            </h4>
                                            <button
                                                onClick={refreshZaiModels}
                                                disabled={zaiModelsLoading || !appConfig.proxy.zai?.api_key}
                                                className="btn btn-ghost btn-xs gap-1"
                                            >
                                                <RefreshCw size={12} className={zaiModelsLoading ? 'animate-spin' : ''} />
                                                {t('proxy.config.zai.models.refresh')}
                                            </button>
                                        </div>

                                        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                                            {['opus', 'sonnet', 'haiku'].map((family) => (
                                                <div key={family} className="space-y-1">
                                                    <label className="text-[10px] text-gray-500 capitalize">{family}</label>
                                                    <div className="flex gap-1">
                                                        {zaiModelOptions.length > 0 && (
                                                            <select
                                                                className="select select-xs select-bordered max-w-[80px]"
                                                                value=""
                                                                onChange={(e) => e.target.value && updateZaiDefaultModels({ [family]: e.target.value })}
                                                            >
                                                                <option value="">Select</option>
                                                                {zaiModelOptions.map(m => <option key={m} value={m}>{m}</option>)}
                                                            </select>
                                                        )}
                                                        <input
                                                            type="text"
                                                            className="input input-xs input-bordered w-full font-mono"
                                                            value={appConfig.proxy.zai?.models?.[family as keyof typeof appConfig.proxy.zai.models] || ''}
                                                            onChange={(e) => updateZaiDefaultModels({ [family]: e.target.value })}
                                                        />
                                                    </div>
                                                </div>
                                            ))}
                                        </div>

                                        <details className="mt-3 group">
                                            <summary className="cursor-pointer text-[10px] text-gray-500 hover:text-blue-500 transition-colors inline-flex items-center gap-1 select-none">
                                                <Settings size={12} />
                                                {t('proxy.config.zai.models.advanced_title')}
                                            </summary>
                                            <div className="mt-2 space-y-2 p-2 bg-gray-50 dark:bg-base-200/50 rounded-lg">
                                                {/* Advanced Mapping Table */}
                                                {Object.entries(zaiModelMapping).map(([from, to]) => (
                                                    <div key={from} className="flex items-center gap-2">
                                                        <div className="flex-1 bg-card px-2 py-1 rounded border border-border text-[10px] font-mono truncate" title={from}>{from}</div>
                                                        <ArrowRight size={10} className="text-gray-400" />
                                                        <div className="flex-[1.5] flex gap-1">
                                                            {zaiModelOptions.length > 0 && (
                                                                <select
                                                                    className="select select-xs select-ghost h-6 min-h-0 px-1"
                                                                    value=""
                                                                    onChange={(e) => e.target.value && upsertZaiModelMapping(from, e.target.value)}
                                                                >
                                                                    <option value="">â–¼</option>
                                                                    {zaiModelOptions.map(m => <option key={m} value={m}>{m}</option>)}
                                                                </select>
                                                            )}
                                                            <input
                                                                type="text"
                                                                className="input input-xs input-bordered w-full font-mono h-6"
                                                                value={to}
                                                                onChange={(e) => upsertZaiModelMapping(from, e.target.value)}
                                                            />
                                                        </div>
                                                        <button onClick={() => removeZaiModelMapping(from)} className="text-gray-400 hover:text-red-500"><Trash2 size={12} /></button>
                                                    </div>
                                                ))}

                                                <div className="flex items-center gap-2 pt-2 border-t border-gray-200/50">
                                                    <input
                                                        className="input input-xs input-bordered flex-1 font-mono"
                                                        placeholder="From (e.g. claude-3-opus)"
                                                        value={zaiNewMappingFrom}
                                                        onChange={e => setZaiNewMappingFrom(e.target.value)}
                                                    />
                                                    <input
                                                        className="input input-xs input-bordered flex-1 font-mono"
                                                        placeholder="To (e.g. glm-4)"
                                                        value={zaiNewMappingTo}
                                                        onChange={e => setZaiNewMappingTo(e.target.value)}
                                                    />
                                                    <button
                                                        className="btn btn-xs btn-primary"
                                                        onClick={() => {
                                                            if (zaiNewMappingFrom && zaiNewMappingTo) {
                                                                upsertZaiModelMapping(zaiNewMappingFrom, zaiNewMappingTo);
                                                                setZaiNewMappingFrom('');
                                                                setZaiNewMappingTo('');
                                                            }
                                                        }}
                                                    >
                                                        <Plus size={12} />
                                                    </button>
                                                </div>
                                            </div>
                                        </details>
                                    </div>
                                </div>
                            </CollapsibleCard>

                            {/* Minimax Dispatcher */}
                            <CollapsibleCard
                                title={t('proxy.config.minimax.title', { defaultValue: 'Minimax' })}
                                icon={<Zap size={18} className="text-purple-500" />}
                                enabled={!!appConfig.proxy.minimax?.enabled}
                                onToggle={(checked) => updateMinimaxGeneralConfig({ enabled: checked })}
                            >
                                <div className="space-y-4">
                                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                                        <div className="space-y-1">
                                            <label className="text-[11px] font-medium text-gray-500 dark:text-gray-400">
                                                {t('proxy.config.minimax.base_url', { defaultValue: 'Base URL' })}
                                            </label>
                                            <input
                                                type="text"
                                                value={appConfig.proxy.minimax?.base_url || 'https://api.minimax.io/v1'}
                                                onChange={(e) => updateMinimaxGeneralConfig({ base_url: e.target.value })}
                                                className="input input-sm input-bordered w-full font-mono text-xs"
                                            />
                                        </div>
                                        <div className="space-y-1">
                                            <label className="text-[11px] font-medium text-gray-500 dark:text-gray-400">
                                                {t('proxy.config.minimax.dispatch_mode', { defaultValue: 'Dispatch Mode' })}
                                            </label>
                                            <select
                                                className="select select-sm select-bordered w-full text-xs"
                                                value={appConfig.proxy.minimax?.dispatch_mode || 'off'}
                                                onChange={(e) => updateMinimaxGeneralConfig({ dispatch_mode: e.target.value as any })}
                                            >
                                                <option value="off">{t('proxy.config.zai.modes.off', { defaultValue: 'Off' })}</option>
                                                <option value="exclusive">{t('proxy.config.zai.modes.exclusive', { defaultValue: 'Exclusive' })}</option>
                                                <option value="pooled">{t('proxy.config.zai.modes.pooled', { defaultValue: 'Pooled' })}</option>
                                                <option value="fallback">{t('proxy.config.zai.modes.fallback', { defaultValue: 'Fallback' })}</option>
                                            </select>
                                        </div>
                                    </div>

                                    <div className="space-y-1">
                                        <label className="text-[11px] font-medium text-gray-500 dark:text-gray-400 flex items-center justify-between">
                                            <span>{t('proxy.config.minimax.api_key', { defaultValue: 'API Key' })}</span>
                                            {!(appConfig.proxy.minimax?.api_key) && (
                                                <span className="text-amber-500 text-[10px] flex items-center gap-1">
                                                    <HelpTooltip text={t('proxy.config.minimax.warning', { defaultValue: 'API key is required to use Minimax' })} />
                                                    {t('common.required', { defaultValue: 'Required' })}
                                                </span>
                                            )}
                                        </label>
                                        <input
                                            type="password"
                                            value={appConfig.proxy.minimax?.api_key || ''}
                                            onChange={(e) => updateMinimaxGeneralConfig({ api_key: e.target.value })}
                                            placeholder="sk-api-..."
                                            className="input input-sm input-bordered w-full font-mono text-xs"
                                        />
                                    </div>

                                    {/* Model Mapping Section */}
                                    <div className="pt-4 border-t border-border">
                                        <div className="flex items-center justify-between mb-3">
                                            <h4 className="text-[11px] font-bold text-gray-400 uppercase tracking-widest">
                                                {t('proxy.config.minimax.models.title', { defaultValue: 'Model Mapping' })}
                                            </h4>
                                            <button
                                                onClick={refreshMinimaxModels}
                                                disabled={minimaxModelsLoading || !appConfig.proxy.minimax?.api_key}
                                                className="btn btn-ghost btn-xs gap-1"
                                            >
                                                <RefreshCw size={12} className={minimaxModelsLoading ? 'animate-spin' : ''} />
                                                {t('proxy.config.minimax.models.refresh', { defaultValue: 'Refresh Models' })}
                                            </button>
                                        </div>

                                        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                                            {['opus', 'sonnet', 'haiku'].map((family) => (
                                                <div key={family} className="space-y-1">
                                                    <label className="text-[10px] text-gray-500 capitalize">{family}</label>
                                                    <div className="flex gap-1">
                                                        {minimaxModelOptions.length > 0 && (
                                                            <select
                                                                className="select select-xs select-bordered max-w-[80px]"
                                                                value=""
                                                                onChange={(e) => e.target.value && updateMinimaxDefaultModels({ [family]: e.target.value })}
                                                            >
                                                                <option value="">Select</option>
                                                                {minimaxModelOptions.map(m => <option key={m} value={m}>{m}</option>)}
                                                            </select>
                                                        )}
                                                        <input
                                                            type="text"
                                                            className="input input-xs input-bordered w-full font-mono"
                                                            value={appConfig.proxy.minimax?.models?.[family as keyof typeof appConfig.proxy.minimax.models] || ''}
                                                            onChange={(e) => updateMinimaxDefaultModels({ [family]: e.target.value })}
                                                        />
                                                    </div>
                                                </div>
                                            ))}
                                        </div>

                                        <details className="mt-3 group">
                                            <summary className="cursor-pointer text-[10px] text-gray-500 hover:text-blue-500 transition-colors inline-flex items-center gap-1 select-none">
                                                <Settings size={12} />
                                                {t('proxy.config.minimax.models.advanced_title', { defaultValue: 'Advanced Model Mapping' })}
                                            </summary>
                                            <div className="mt-2 space-y-2 p-2 bg-gray-50 dark:bg-base-200/50 rounded-lg">
                                                {/* Advanced Mapping Table */}
                                                {Object.entries(minimaxModelMapping).map(([from, to]) => (
                                                    <div key={from} className="flex items-center gap-2">
                                                        <div className="flex-1 bg-card px-2 py-1 rounded border border-border text-[10px] font-mono truncate" title={from}>{from}</div>
                                                        <ArrowRight size={10} className="text-gray-400" />
                                                        <div className="flex-[1.5] flex gap-1">
                                                            {minimaxModelOptions.length > 0 && (
                                                                <select
                                                                    className="select select-xs select-ghost h-6 min-h-0 px-1"
                                                                    value=""
                                                                    onChange={(e) => e.target.value && upsertMinimaxModelMapping(from, e.target.value)}
                                                                >
                                                                    <option value="">â–¼</option>
                                                                    {minimaxModelOptions.map(m => <option key={m} value={m}>{m}</option>)}
                                                                </select>
                                                            )}
                                                            <input
                                                                type="text"
                                                                className="input input-xs input-bordered w-full font-mono h-6"
                                                                value={to}
                                                                onChange={(e) => upsertMinimaxModelMapping(from, e.target.value)}
                                                            />
                                                        </div>
                                                        <button onClick={() => removeMinimaxModelMapping(from)} className="text-gray-400 hover:text-red-500"><Trash2 size={12} /></button>
                                                    </div>
                                                ))}

                                                <div className="flex items-center gap-2 pt-2 border-t border-gray-200/50">
                                                    <input
                                                        className="input input-xs input-bordered flex-1 font-mono"
                                                        placeholder="From (e.g. claude-3-opus)"
                                                        value={minimaxNewMappingFrom}
                                                        onChange={e => setMinimaxNewMappingFrom(e.target.value)}
                                                    />
                                                    <input
                                                        className="input input-xs input-bordered flex-1 font-mono"
                                                        placeholder="To (e.g. MiniMax-Text-01)"
                                                        value={minimaxNewMappingTo}
                                                        onChange={e => setMinimaxNewMappingTo(e.target.value)}
                                                    />
                                                    <button
                                                        className="btn btn-xs btn-primary"
                                                        onClick={() => {
                                                            if (minimaxNewMappingFrom && minimaxNewMappingTo) {
                                                                upsertMinimaxModelMapping(minimaxNewMappingFrom, minimaxNewMappingTo);
                                                                setMinimaxNewMappingFrom('');
                                                                setMinimaxNewMappingTo('');
                                                            }
                                                        }}
                                                    >
                                                        <Plus size={12} />
                                                    </button>
                                                </div>
                                            </div>
                                        </details>
                                    </div>
                                </div>
                            </CollapsibleCard>

                            <CollapsibleCard
                                title={t('proxy.config.zai.mcp.title')}
                                icon={<Puzzle size={18} className="text-blue-500" />}
                                enabled={!!appConfig.proxy.zai?.mcp?.enabled}
                                onToggle={(checked) => updateZaiGeneralConfig({ mcp: { ...(appConfig.proxy.zai?.mcp || {}), enabled: checked } as any })}
                                rightElement={
                                    <div className="flex gap-2 text-[10px] text-gray-400">
                                        {['web_search', 'web_reader', 'vision'].map(f =>
                                            appConfig.proxy.zai?.mcp?.[(f + '_enabled') as keyof typeof appConfig.proxy.zai.mcp] && (
                                                <span key={f} className="bg-gray-100 dark:bg-base-200 px-1.5 py-0.5 rounded text-gray-600 dark:text-gray-400">
                                                    {t(`proxy.config.zai.mcp.${f}`).split(' ')[0]}
                                                </span>
                                            )
                                        )}
                                    </div>
                                }
                            >
                                <div className="space-y-3">
                                    <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                                        <label className="flex items-center gap-2 border border-border p-2 rounded-lg cursor-pointer hover:bg-gray-50 dark:hover:bg-base-200/50 transition-colors">
                                            <input
                                                type="checkbox"
                                                className="checkbox checkbox-xs checkbox-primary rounded-md"
                                                checked={!!appConfig.proxy.zai?.mcp?.web_search_enabled}
                                                onChange={(e) => updateZaiGeneralConfig({ mcp: { ...(appConfig.proxy.zai?.mcp || {}), web_search_enabled: e.target.checked } as any })}
                                            />
                                            <span className="text-xs">{t('proxy.config.zai.mcp.web_search')}</span>
                                        </label>
                                        <label className="flex items-center gap-2 border border-border p-2 rounded-lg cursor-pointer hover:bg-gray-50 dark:hover:bg-base-200/50 transition-colors">
                                            <input
                                                type="checkbox"
                                                className="checkbox checkbox-xs checkbox-primary rounded-md"
                                                checked={!!appConfig.proxy.zai?.mcp?.web_reader_enabled}
                                                onChange={(e) => updateZaiGeneralConfig({ mcp: { ...(appConfig.proxy.zai?.mcp || {}), web_reader_enabled: e.target.checked } as any })}
                                            />
                                            <span className="text-xs">{t('proxy.config.zai.mcp.web_reader')}</span>
                                        </label>
                                        <label className="flex items-center gap-2 border border-border p-2 rounded-lg cursor-pointer hover:bg-gray-50 dark:hover:bg-base-200/50 transition-colors">
                                            <input
                                                type="checkbox"
                                                className="checkbox checkbox-xs checkbox-primary rounded-md"
                                                checked={!!appConfig.proxy.zai?.mcp?.vision_enabled}
                                                onChange={(e) => updateZaiGeneralConfig({ mcp: { ...(appConfig.proxy.zai?.mcp || {}), vision_enabled: e.target.checked } as any })}
                                            />
                                            <span className="text-xs">{t('proxy.config.zai.mcp.vision')}</span>
                                        </label>
                                    </div>

                                    {appConfig.proxy.zai?.mcp?.enabled && (
                                        <div className="bg-gray-50 dark:bg-base-200/50 rounded-lg p-3 text-[10px] font-mono text-gray-500">
                                            <div className="mb-1 font-bold text-gray-400 uppercase tracking-wider">{t('proxy.config.zai.mcp.local_endpoints')}</div>
                                            <div className="space-y-0.5 select-all">
                                                {appConfig.proxy.zai?.mcp?.web_search_enabled && <div>http://127.0.0.1:{status.running ? status.port : (appConfig.proxy.port || 9000)}/mcp/web_search_prime/mcp</div>}
                                                {appConfig.proxy.zai?.mcp?.web_reader_enabled && <div>http://127.0.0.1:{status.running ? status.port : (appConfig.proxy.port || 9000)}/mcp/web_reader/mcp</div>}
                                                {appConfig.proxy.zai?.mcp?.vision_enabled && <div>http://127.0.0.1:{status.running ? status.port : (appConfig.proxy.port || 9000)}/mcp/zai-mcp-server/mcp</div>}
                                            </div>
                                        </div>
                                    )}
                                </div>
                            </CollapsibleCard>

                            {/* Account Scheduling & Rotation */}
                            <CollapsibleCard
                                title={t('proxy.config.scheduling.title')}
                                icon={<RefreshCw size={18} className="text-indigo-500" />}
                            >
                                <div className="space-y-4">
                                    <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
                                        <div className="space-y-3">
                                            <div className="flex items-center justify-between">
                                                <label className="text-xs font-medium text-gray-700 dark:text-gray-300 inline-flex items-center gap-1">
                                                    {t('proxy.config.scheduling.mode')}
                                                    <HelpTooltip
                                                        text={t('proxy.config.scheduling.mode_tooltip')}
                                                        placement="right"
                                                    />
                                                </label>
                                                <button
                                                    onClick={handleClearSessionBindings}
                                                    className="text-[10px] text-indigo-500 hover:text-indigo-600 transition-colors flex items-center gap-1"
                                                >
                                                    <Trash2 size={12} />
                                                    {t('proxy.config.scheduling.clear_bindings')}
                                                </button>
                                            </div>
                                            <div className="grid grid-cols-1 gap-2">
                                                {(['CacheFirst', 'Balance', 'PerformanceFirst'] as const).map(mode => (
                                                    <label
                                                        key={mode}
                                                        className={`flex items-start gap-3 p-3 rounded-xl border cursor-pointer transition-all duration-200 ${(appConfig.proxy.scheduling?.mode || 'Balance') === mode
                                                            ? 'border-indigo-500 bg-indigo-50/30 dark:bg-indigo-900/10'
                                                            : 'border-border hover:border-indigo-200'
                                                            }`}
                                                    >
                                                        <input
                                                            type="radio"
                                                            className="radio radio-xs radio-primary mt-1"
                                                            checked={(appConfig.proxy.scheduling?.mode || 'Balance') === mode}
                                                            onChange={() => updateSchedulingConfig({ mode })}
                                                        />
                                                        <div className="space-y-1">
                                                            <div className="text-xs font-bold text-card-foreground">
                                                                {t(`proxy.config.scheduling.modes.${mode}`)}
                                                            </div>
                                                            <div className="text-[10px] text-gray-500 line-clamp-2">
                                                                {t(`proxy.config.scheduling.modes_desc.${mode}`, {
                                                                    defaultValue: mode === 'CacheFirst' ? 'Binds session to account, waits precisely if limited (Maximizes Prompt Cache hits).' :
                                                                        mode === 'Balance' ? 'Binds session, auto-switches to available account if limited (Balanced cache & availability).' :
                                                                            'No session binding, pure round-robin rotation (Best for high concurrency).'
                                                                })}
                                                            </div>
                                                        </div>
                                                    </label>
                                                ))}
                                            </div>
                                        </div>

                                        <div className="space-y-4 pt-1">
                                            <div className="bg-gray-50 dark:bg-base-200/50 rounded-xl p-4 border border-border">
                                                <div className="flex items-center justify-between mb-2">
                                                    <label className="text-xs font-medium text-gray-700 dark:text-gray-300 inline-flex items-center gap-1">
                                                        {t('proxy.config.scheduling.max_wait')}
                                                        <HelpTooltip text={t('proxy.config.scheduling.max_wait_tooltip')} />
                                                    </label>
                                                    <span className="text-xs font-mono text-indigo-600 font-bold">
                                                        {appConfig.proxy.scheduling?.max_wait_seconds || 60}s
                                                    </span>
                                                </div>
                                                <input
                                                    type="range"
                                                    min="0"
                                                    max="300"
                                                    step="10"
                                                    disabled={(appConfig.proxy.scheduling?.mode || 'Balance') !== 'CacheFirst'}
                                                    className="range range-indigo range-xs"
                                                    value={appConfig.proxy.scheduling?.max_wait_seconds || 60}
                                                    onChange={(e) => updateSchedulingConfig({ max_wait_seconds: parseInt(e.target.value) })}
                                                />
                                                <div className="flex justify-between px-1 mt-1 text-[10px] text-gray-400 font-mono">
                                                    <span>0s</span>
                                                    <span>300s</span>
                                                </div>
                                            </div>

                                            <div className="p-3 bg-amber-50 dark:bg-amber-900/10 border border-amber-100 dark:border-amber-900/20 rounded-xl">
                                                <p className="text-[10px] text-amber-700 dark:text-amber-500 leading-relaxed">
                                                    <strong>{t('common.info')}:</strong> {t('proxy.config.scheduling.subtitle')}
                                                </p>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </CollapsibleCard>
                        </div>
                    )
                }

                {/* æ¨¡åž‹è·¯ç”±ä¸­å¿ƒ */}
                {
                    appConfig && (
                        <div className="bg-card rounded-xl shadow-sm border border-border overflow-hidden">
                            <div className="px-4 py-2.5 border-b border-border bg-gray-50/50 dark:bg-gradient-to-b from-[#2a2a2a] to-[#1a1a1a]">
                                <div className="flex items-center justify-between">
                                    <div>
                                        <h2 className="text-base font-bold flex items-center gap-2 text-card-foreground">
                                            <BrainCircuit size={18} className="text-blue-500" />
                                            {t('proxy.router.title')}
                                        </h2>
                                        <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                                            {t('proxy.router.subtitle')}
                                        </p>
                                    </div>
                                    <button
                                        onClick={handleResetMapping}
                                        className="px-3 py-1 rounded-lg text-xs font-medium transition-colors flex items-center gap-2 bg-card border border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-base-200 hover:text-blue-600 dark:hover:text-blue-400 hover:border-blue-200 dark:hover:border-blue-800 shadow-sm"
                                    >
                                        <RefreshCw size={14} />
                                        {t('proxy.router.reset_mapping')}
                                    </button>
                                </div>
                            </div>

                            <div className="p-3 space-y-3">
                                {/* åˆ†ç»„æ˜ å°„åŒºåŸŸ */}
                                <div>
                                    <h3 className="text-[10px] font-bold text-gray-400 uppercase tracking-widest mb-2 flex items-center gap-2">
                                        <Layers size={14} /> {t('proxy.router.group_title')}
                                    </h3>
                                    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-3">
                                        {/* Claude 4.5 ç³»åˆ— */}
                                        <div className="bg-gradient-to-br from-orange-50 to-amber-50 dark:from-orange-900/10 dark:to-amber-900/10 p-3 rounded-xl border border-orange-100 dark:border-orange-800/30 relative overflow-hidden group hover:border-orange-400 transition-all duration-300">
                                            <div className="flex items-center gap-3 mb-3">
                                                <div className="w-8 h-8 rounded-lg bg-orange-500 flex items-center justify-center text-white shadow-lg shadow-orange-500/30">
                                                    <BrainCircuit size={16} />
                                                </div>
                                                <div>
                                                    <div className="text-xs font-bold text-card-foreground">{t('proxy.router.groups.claude_45.name')}</div>
                                                    <div className="text-[10px] text-gray-500 line-clamp-1">{t('proxy.router.groups.claude_45.desc')}</div>
                                                </div>
                                            </div>
                                            <select
                                                className="select select-sm select-bordered w-full font-mono text-[11px] bg-card/80 backdrop-blur-sm"
                                                value={appConfig.proxy.anthropic_mapping?.["claude-4.5-series"] || ""}
                                                onChange={(e) => handleMappingUpdate('anthropic', 'claude-4.5-series', e.target.value)}
                                            >
                                                <option value="gemini-3-pro-high">gemini-3-pro-high{t('proxy.router.default_suffix', ' (Default)')}</option>
                                                <optgroup label="Claude 4.5">
                                                    <option value="claude-opus-4-5-thinking">claude-opus-4-5-thinking</option>
                                                    <option value="claude-sonnet-4-5">claude-sonnet-4-5</option>
                                                    <option value="claude-sonnet-4-5-thinking">claude-sonnet-4-5-thinking</option>
                                                </optgroup>
                                                <optgroup label="Gemini 3">
                                                    <option value="gemini-3-pro-high">gemini-3-pro-high</option>
                                                    <option value="gemini-3-pro-low">gemini-3-pro-low</option>
                                                    <option value="gemini-3-flash">gemini-3-flash</option>
                                                </optgroup>
                                                <optgroup label="Gemini 2.5">
                                                    <option value="gemini-2.5-pro">gemini-2.5-pro</option>
                                                    <option value="gemini-2.5-flash">gemini-2.5-flash</option>
                                                    <option value="gemini-2.5-flash-thinking">gemini-2.5-flash-thinking</option>
                                                    <option value="gemini-2.5-flash-lite">gemini-2.5-flash-lite</option>
                                                </optgroup>
                                            </select>
                                        </div>

                                        {/* Claude 3.5 ç³»åˆ— */}
                                        <div className="bg-gradient-to-br from-purple-50 to-pink-50 dark:from-purple-900/10 dark:to-pink-900/10 p-3 rounded-xl border border-purple-100 dark:border-purple-800/30 relative overflow-hidden group hover:border-purple-400 transition-all duration-300">
                                            <div className="flex items-center gap-3 mb-3">
                                                <div className="w-8 h-8 rounded-lg bg-purple-600 flex items-center justify-center text-white shadow-lg shadow-purple-500/30">
                                                    <Puzzle size={16} />
                                                </div>
                                                <div>
                                                    <div className="text-xs font-bold text-card-foreground">{t('proxy.router.groups.claude_35.name')}</div>
                                                    <div className="text-[10px] text-gray-500 line-clamp-1">{t('proxy.router.groups.claude_35.desc')}</div>
                                                </div>
                                            </div>
                                            <select
                                                className="select select-sm select-bordered w-full font-mono text-[11px] bg-card/80 backdrop-blur-sm"
                                                value={appConfig.proxy.anthropic_mapping?.["claude-3.5-series"] || ""}
                                                onChange={(e) => handleMappingUpdate('anthropic', 'claude-3.5-series', e.target.value)}
                                            >
                                                <option value="claude-sonnet-4-5-thinking">claude-sonnet-4-5-thinking{t('proxy.router.default_suffix', ' (Default)')}</option>
                                                <optgroup label="Claude 4.5">
                                                    <option value="claude-opus-4-5-thinking">claude-opus-4-5-thinking</option>
                                                    <option value="claude-sonnet-4-5">claude-sonnet-4-5</option>
                                                    <option value="claude-sonnet-4-5-thinking">claude-sonnet-4-5-thinking</option>
                                                </optgroup>
                                                <optgroup label="Gemini 3">
                                                    <option value="gemini-3-pro-high">gemini-3-pro-high</option>
                                                    <option value="gemini-3-pro-low">gemini-3-pro-low</option>
                                                    <option value="gemini-3-flash">gemini-3-flash</option>
                                                </optgroup>
                                                <optgroup label="Gemini 2.5">
                                                    <option value="gemini-2.5-pro">gemini-2.5-pro</option>
                                                    <option value="gemini-2.5-flash">gemini-2.5-flash</option>
                                                    <option value="gemini-2.5-flash-thinking">gemini-2.5-flash-thinking</option>
                                                    <option value="gemini-2.5-flash-lite">gemini-2.5-flash-lite</option>
                                                </optgroup>
                                            </select>
                                        </div>

                                        {/* GPT-4 ç³»åˆ— */}
                                        <div className="bg-gradient-to-br from-indigo-50 to-blue-50 dark:from-indigo-900/10 dark:to-blue-900/10 p-3 rounded-xl border border-indigo-100 dark:border-indigo-800/30 relative overflow-hidden group hover:border-indigo-400 transition-all duration-300">
                                            <div className="flex items-center gap-3 mb-3">
                                                <div className="w-8 h-8 rounded-lg bg-indigo-600 flex items-center justify-center text-white shadow-lg shadow-indigo-500/30">
                                                    <Zap size={16} />
                                                </div>
                                                <div>
                                                    <div className="text-xs font-bold text-card-foreground">{t('proxy.router.groups.gpt_4.name')}</div>
                                                    <div className="text-[10px] text-gray-500 line-clamp-1">{t('proxy.router.groups.gpt_4.desc')}</div>
                                                </div>
                                            </div>
                                            <select
                                                className="select select-sm select-bordered w-full font-mono text-[11px] bg-card/80 backdrop-blur-sm"
                                                value={appConfig.proxy.openai_mapping?.["gpt-4-series"] || ""}
                                                onChange={(e) => handleMappingUpdate('openai', 'gpt-4-series', e.target.value)}
                                            >
                                                <option value="gemini-3-pro-high">gemini-3-pro-high{t('proxy.router.default_suffix', ' (Default)')}</option>
                                                <optgroup label="Gemini 3 (Recommended)">
                                                    <option value="gemini-3-pro-high">gemini-3-pro-high (High Quality)</option>
                                                    <option value="gemini-3-pro-low">gemini-3-pro-low (Balanced)</option>
                                                    <option value="gemini-3-flash">gemini-3-flash (Fast)</option>
                                                </optgroup>
                                            </select>
                                            <p className="mt-1 text-[9px] text-indigo-500">{t('proxy.router.gemini3_only_warning')}</p>
                                        </div>

                                        {/* GPT-4o / 3.5 ç³»åˆ— */}
                                        <div className="bg-gradient-to-br from-emerald-50 to-green-50 dark:from-emerald-900/10 dark:to-green-900/10 p-3 rounded-xl border border-emerald-100 dark:border-emerald-800/30 relative overflow-hidden group hover:border-emerald-400 transition-all duration-300">
                                            <div className="flex items-center gap-3 mb-3">
                                                <div className="w-8 h-8 rounded-lg bg-emerald-600 flex items-center justify-center text-white shadow-lg shadow-emerald-500/30">
                                                    <Wind size={16} />
                                                </div>
                                                <div>
                                                    <div className="text-xs font-bold text-card-foreground">{t('proxy.router.groups.gpt_4o.name')}</div>
                                                    <div className="text-[10px] text-gray-500 line-clamp-1">{t('proxy.router.groups.gpt_4o.desc')}</div>
                                                </div>
                                            </div>
                                            <select
                                                className="select select-sm select-bordered w-full font-mono text-[11px] bg-card/80 backdrop-blur-sm"
                                                value={appConfig.proxy.openai_mapping?.["gpt-4o-series"] || ""}
                                                onChange={(e) => handleMappingUpdate('openai', 'gpt-4o-series', e.target.value)}
                                            >
                                                <option value="gemini-3-flash">gemini-3-flash{t('proxy.router.default_suffix', ' (Default)')}</option>
                                                <optgroup label="Gemini 3 (Recommended)">
                                                    <option value="gemini-3-flash">gemini-3-flash (Fast)</option>
                                                    <option value="gemini-3-pro-high">gemini-3-pro-high (High Quality)</option>
                                                    <option value="gemini-3-pro-low">gemini-3-pro-low (Balanced)</option>
                                                </optgroup>
                                            </select>
                                            <p className="mt-1 text-[9px] text-emerald-600">{t('proxy.router.gemini3_only_warning')}</p>
                                        </div>

                                        {/* GPT-5 ç³»åˆ— */}
                                        <div className="bg-gradient-to-br from-amber-50 to-orange-50 dark:from-amber-900/10 dark:to-orange-900/10 p-3 rounded-xl border border-amber-100 dark:border-amber-800/30 relative overflow-hidden group hover:border-amber-400 transition-all duration-300">
                                            <div className="flex items-center gap-3 mb-3">
                                                <div className="w-8 h-8 rounded-lg bg-amber-600 flex items-center justify-center text-white shadow-lg shadow-amber-500/30">
                                                    <Zap size={16} />
                                                </div>
                                                <div>
                                                    <div className="text-xs font-bold text-card-foreground">{t('proxy.router.groups.gpt_5.name')}</div>
                                                    <div className="text-[10px] text-gray-500 line-clamp-1">{t('proxy.router.groups.gpt_5.desc')}</div>
                                                </div>
                                            </div>
                                            <select
                                                className="select select-sm select-bordered w-full font-mono text-[11px] bg-card/80 backdrop-blur-sm"
                                                value={appConfig.proxy.openai_mapping?.["gpt-5-series"] || ""}
                                                onChange={(e) => handleMappingUpdate('openai', 'gpt-5-series', e.target.value)}
                                            >
                                                <option value="gemini-3-flash">gemini-3-flash{t('proxy.router.default_suffix', ' (Default)')}</option>
                                                <optgroup label="Gemini 3 (Recommended)">
                                                    <option value="gemini-3-flash">gemini-3-flash (Fast)</option>
                                                    <option value="gemini-3-pro-high">gemini-3-pro-high (High Quality)</option>
                                                    <option value="gemini-3-pro-low">gemini-3-pro-low (Balanced)</option>
                                                </optgroup>
                                            </select>
                                            <p className="mt-1 text-[9px] text-amber-600">{t('proxy.router.gemini3_only_warning')}</p>
                                        </div>
                                    </div>
                                </div>

                                {/* ç²¾ç¡®æ˜ å°„ç®¡ç† */}
                                <div className="pt-4 border-t border-border">
                                    <div className="flex items-center justify-between mb-3">
                                        <h3 className="text-[10px] font-bold text-gray-400 uppercase tracking-widest flex items-center gap-2">
                                            <ArrowRight size={14} /> {t('proxy.router.expert_title')}
                                        </h3>
                                    </div>

                                    {/* ðŸ’¡ Haiku Optimization Tip */}
                                    <div className="mb-4 p-3 bg-blue-50/50 dark:bg-blue-900/10 rounded-lg border border-blue-100 dark:border-blue-800/30">
                                        <div className="flex items-center justify-between gap-3">
                                            <div className="flex items-center gap-2 flex-1">
                                                <Sparkles size={14} className="text-blue-500 flex-shrink-0" />
                                                <p className="text-[11px] text-gray-600 dark:text-gray-400">
                                                    <span className="font-medium text-blue-600 dark:text-blue-400">ðŸ’° Cost Saving Tip:</span>
                                                    {' '}Claude CLI uses <code className="px-1 py-0.5 bg-gray-100 dark:bg-gray-800 rounded text-[10px] font-mono">claude-haiku-4-5-20251001</code> for background tasks. Map it to a cheaper Flash model to save ~95% cost.
                                                </p>
                                            </div>
                                            <button
                                                onClick={handleAddHaikuOptimization}
                                                className="btn btn-ghost btn-xs gap-1.5 text-blue-600 dark:text-blue-400 hover:bg-blue-100 dark:hover:bg-blue-900/30 border border-blue-200 dark:border-blue-800 whitespace-nowrap flex-shrink-0"
                                            >
                                                <Plus size={12} />
                                                Quick Optimize
                                            </button>
                                        </div>
                                    </div>
                                    <div className="flex flex-col lg:flex-row gap-6">
                                        {/* æ·»åŠ æ˜ å°„è¡¨å• */}
                                        <div className="flex-1 flex flex-col gap-3">
                                            <div className="flex items-center gap-2 text-[10px] font-bold text-gray-400 uppercase tracking-wider">
                                                <Target size={12} />
                                                <span>{t('proxy.router.add_mapping')}</span>
                                            </div>
                                            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                                                <input
                                                    id="custom-key"
                                                    type="text"
                                                    placeholder="Original (e.g. gpt-4)"
                                                    className="input input-xs input-bordered w-full font-mono text-[11px] bg-card border border-gray-200 dark:border-gray-700 shadow-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500 transition-all placeholder:text-gray-400"
                                                />
                                                <input
                                                    id="custom-val"
                                                    type="text"
                                                    placeholder="Target (e.g. gemini-2.5-pro)"
                                                    className="input input-xs input-bordered w-full font-mono text-[11px] bg-card border border-gray-200 dark:border-gray-700 shadow-sm focus:border-blue-500 focus:ring-1 focus:ring-blue-500 transition-all placeholder:text-gray-400"
                                                />
                                            </div>
                                            <button
                                                className="btn btn-xs w-full gap-2 shadow-md hover:shadow-lg transition-all bg-blue-600 hover:bg-blue-700 text-white border-none"
                                                onClick={() => {
                                                    const k = (document.getElementById('custom-key') as HTMLInputElement).value;
                                                    const v = (document.getElementById('custom-val') as HTMLInputElement).value;
                                                    if (k && v) {
                                                        handleMappingUpdate('custom', k, v);
                                                        (document.getElementById('custom-key') as HTMLInputElement).value = '';
                                                        (document.getElementById('custom-val') as HTMLInputElement).value = '';
                                                    }
                                                }}
                                            >
                                                <Plus size={14} />
                                                {t('common.add')}
                                            </button>
                                        </div>
                                        {/* è‡ªå®šä¹‰ç²¾ç¡®æ˜ å°„è¡¨æ ¼ */}
                                        <div className="flex-1 min-w-[300px] flex flex-col">
                                            <div className="flex items-center justify-between mb-2">
                                                <span className="text-[10px] font-bold text-gray-400 uppercase tracking-wider">
                                                    {t('proxy.router.current_list')}
                                                </span>
                                            </div>
                                            <div className="flex-1 overflow-y-auto max-h-[140px] border border-border rounded-lg bg-gray-50/30 dark:bg-base-200/30" data-custom-mapping-list>
                                                <table className="table table-xs w-full bg-card">
                                                    <thead className="sticky top-0 bg-gray-50/95 dark:bg-gradient-to-b from-[#2a2a2a] to-[#1a1a1a] backdrop-blur shadow-sm z-10 text-gray-500 dark:text-gray-400">
                                                        <tr>
                                                            <th className="text-[10px] py-2 font-medium">{t('proxy.router.original_id')}</th>
                                                            <th className="text-[10px] py-2 font-medium">{t('proxy.router.route_to')}</th>
                                                            <th className="text-[10px] w-12 text-center py-2 font-medium">{t('common.action')}</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody className="font-mono text-[10px]">
                                                        {appConfig.proxy.custom_mapping && Object.entries(appConfig.proxy.custom_mapping).length > 0 ? (
                                                            Object.entries(appConfig.proxy.custom_mapping).map(([key, val]) => (
                                                                <tr key={key} className="hover:bg-gray-100 dark:hover:bg-base-300 transition-colors">
                                                                    <td className="font-bold text-blue-600 dark:text-blue-400">{key}</td>
                                                                    <td>{val}</td>
                                                                    <td className="text-center">
                                                                        <button
                                                                            className="btn btn-ghost btn-xs text-error p-0 h-auto min-h-0"
                                                                            onClick={() => handleRemoveCustomMapping(key)}
                                                                        >
                                                                            <Trash2 size={12} />
                                                                        </button>
                                                                    </td>
                                                                </tr>
                                                            ))
                                                        ) : (
                                                            <tr>
                                                                <td colSpan={3} className="text-center py-2 text-gray-400 italic">{t('proxy.router.no_custom_mapping')}</td>
                                                            </tr>
                                                        )}
                                                    </tbody>
                                                </table>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    )
                }
                {/* å¤šåè®®æ”¯æŒä¿¡æ¯ */}
                {
                    appConfig && status.running && (
                        <div className="bg-card rounded-xl shadow-sm border border-border overflow-hidden">
                            <div className="p-3">
                                <div className="flex items-center gap-3 mb-3">
                                    <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-blue-500 to-purple-600 flex items-center justify-center shadow-md">
                                        <Code size={16} className="text-white" />
                                    </div>
                                    <div>
                                        <h3 className="text-base font-bold text-card-foreground">
                                            ðŸ”— {t('proxy.multi_protocol.title')}
                                        </h3>
                                        <p className="text-[10px] text-gray-500 dark:text-gray-400">
                                            {t('proxy.multi_protocol.subtitle')}
                                        </p>
                                    </div>
                                </div>

                                <p className="text-xs text-gray-700 dark:text-gray-300 mb-4 leading-relaxed">
                                    {t('proxy.multi_protocol.description')}
                                </p>

                                <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                                    {/* OpenAI Card */}
                                    <div
                                        className={`p-3 rounded-xl border-2 transition-all cursor-pointer ${selectedProtocol === 'openai' ? 'border-blue-500 bg-blue-50/30 dark:bg-blue-900/10' : 'border-border hover:border-blue-200'}`}
                                        onClick={() => setSelectedProtocol('openai')}
                                    >
                                        <div className="flex items-center justify-between mb-2">
                                            <span className="text-xs font-bold text-blue-600">{t('proxy.multi_protocol.openai_label')}</span>
                                            <button onClick={(e) => { e.stopPropagation(); copyToClipboard(`${status.base_url}/v1`, 'openai'); }} className="btn btn-ghost btn-xs">
                                                {copied === 'openai' ? <CheckCircle size={14} /> : <div className="flex items-center gap-1 text-[10px]"><Copy size={12} /> Base</div>}
                                            </button>
                                        </div>
                                        <div className="space-y-1">
                                            <div className="flex items-center justify-between hover:bg-black/5 dark:hover:bg-white/5 rounded p-0.5 group">
                                                <code className="text-[10px] opacity-70">/v1/chat/completions</code>
                                                <button onClick={(e) => { e.stopPropagation(); copyToClipboard(`${status.base_url}/v1/chat/completions`, 'openai-chat'); }} className="opacity-0 group-hover:opacity-100 transition-opacity">
                                                    {copied === 'openai-chat' ? <CheckCircle size={10} className="text-green-500" /> : <Copy size={10} />}
                                                </button>
                                            </div>
                                            <div className="flex items-center justify-between hover:bg-black/5 dark:hover:bg-white/5 rounded p-0.5 group">
                                                <code className="text-[10px] opacity-70">/v1/completions</code>
                                                <button onClick={(e) => { e.stopPropagation(); copyToClipboard(`${status.base_url}/v1/completions`, 'openai-compl'); }} className="opacity-0 group-hover:opacity-100 transition-opacity">
                                                    {copied === 'openai-compl' ? <CheckCircle size={10} className="text-green-500" /> : <Copy size={10} />}
                                                </button>
                                            </div>
                                            <div className="flex items-center justify-between hover:bg-black/5 dark:hover:bg-white/5 rounded p-0.5 group">
                                                <code className="text-[10px] opacity-70 font-bold text-blue-500">/v1/responses (Codex)</code>
                                                <button onClick={(e) => { e.stopPropagation(); copyToClipboard(`${status.base_url}/v1/responses`, 'openai-resp'); }} className="opacity-0 group-hover:opacity-100 transition-opacity">
                                                    {copied === 'openai-resp' ? <CheckCircle size={10} className="text-green-500" /> : <Copy size={10} />}
                                                </button>
                                            </div>
                                        </div>
                                    </div>

                                    {/* Anthropic Card */}
                                    <div
                                        className={`p-3 rounded-xl border-2 transition-all cursor-pointer ${selectedProtocol === 'anthropic' ? 'border-purple-500 bg-purple-50/30 dark:bg-purple-900/10' : 'border-border hover:border-purple-200'}`}
                                        onClick={() => setSelectedProtocol('anthropic')}
                                    >
                                        <div className="flex items-center justify-between mb-2">
                                            <span className="text-xs font-bold text-purple-600">{t('proxy.multi_protocol.anthropic_label')}</span>
                                            <button onClick={(e) => { e.stopPropagation(); copyToClipboard(`${status.base_url}/v1/messages`, 'anthropic'); }} className="btn btn-ghost btn-xs">
                                                {copied === 'anthropic' ? <CheckCircle size={14} /> : <Copy size={14} />}
                                            </button>
                                        </div>
                                        <code className="text-[10px] block truncate bg-black/5 dark:bg-white/5 p-1 rounded">/v1/messages</code>
                                    </div>

                                    {/* Gemini Card */}
                                    <div
                                        className={`p-3 rounded-xl border-2 transition-all cursor-pointer ${selectedProtocol === 'gemini' ? 'border-green-500 bg-green-50/30 dark:bg-green-900/10' : 'border-border hover:border-green-200'}`}
                                        onClick={() => setSelectedProtocol('gemini')}
                                    >
                                        <div className="flex items-center justify-between mb-2">
                                            <span className="text-xs font-bold text-green-600">{t('proxy.multi_protocol.gemini_label')}</span>
                                            <button onClick={(e) => { e.stopPropagation(); copyToClipboard(`${status.base_url}/v1beta/models`, 'gemini'); }} className="btn btn-ghost btn-xs">
                                                {copied === 'gemini' ? <CheckCircle size={14} /> : <Copy size={14} />}
                                            </button>
                                        </div>
                                        <code className="text-[10px] block truncate bg-black/5 dark:bg-white/5 p-1 rounded">/v1beta/models/...</code>
                                    </div>
                                </div>
                            </div>
                        </div>
                    )
                }

                {/* æ”¯æŒæ¨¡åž‹ä¸Žé›†æˆ */}
                {
                    appConfig && (
                        <div className="bg-card rounded-xl shadow-sm border border-border overflow-hidden mt-4">
                            <div className="px-4 py-2.5 border-b border-border bg-gray-50/50 dark:bg-gradient-to-b from-[#2a2a2a] to-[#1a1a1a]">
                                <h2 className="text-base font-bold text-card-foreground flex items-center gap-2">
                                    <Terminal size={18} />
                                    {t('proxy.supported_models.title')}
                                </h2>
                            </div>

                            <div className="grid grid-cols-1 lg:grid-cols-3 gap-0 lg:divide-x dark:divide-gray-700">
                                {/* å·¦ä¾§ï¼šæ¨¡åž‹åˆ—è¡¨ */}
                                <div className="col-span-2 p-0">
                                    <div className="overflow-x-auto">
                                        <table className="table w-full">
                                            <thead className="bg-gray-50/50 dark:bg-gradient-to-b from-[#2a2a2a] to-[#1a1a1a] text-gray-500 dark:text-gray-400">
                                                <tr>
                                                    <th className="w-10 pl-3"></th>
                                                    <th className="text-[11px] font-medium">{t('proxy.supported_models.model_name')}</th>
                                                    <th className="text-[11px] font-medium">{t('proxy.supported_models.model_id')}</th>
                                                    <th className="text-[11px] hidden sm:table-cell font-medium">{t('proxy.supported_models.description')}</th>
                                                    <th className="text-[11px] w-20 text-center font-medium">{t('proxy.supported_models.action')}</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {filteredModels.map((m) => (
                                                    <tr
                                                        key={m.id}
                                                        className={`hover:bg-blue-50/50 dark:hover:bg-blue-900/10 cursor-pointer transition-colors ${selectedModelId === m.id ? 'bg-blue-50/80 dark:bg-blue-900/20' : ''}`}
                                                        onClick={() => setSelectedModelId(m.id)}
                                                    >
                                                        <td className="pl-4 text-blue-500">{m.icon}</td>
                                                        <td className="font-bold text-xs">{m.name}</td>
                                                        <td className="font-mono text-[10px] text-gray-500">{m.id}</td>
                                                        <td className="text-[10px] text-gray-400 hidden sm:table-cell">{m.desc}</td>
                                                        <td className="text-center">
                                                            <button
                                                                className="btn btn-ghost btn-xs text-blue-500"
                                                                onClick={(e) => {
                                                                    e.stopPropagation();
                                                                    copyToClipboard(m.id, `model-${m.id}`);
                                                                }}
                                                            >
                                                                {copied === `model-${m.id}` ? <CheckCircle size={14} /> : <div className="flex items-center gap-1 text-[10px]"><Copy size={12} /> Copy</div>}
                                                            </button>
                                                        </td>
                                                    </tr>
                                                ))}
                                            </tbody>
                                        </table>
                                    </div>
                                </div>

                                {/* å³ä¾§ï¼šä»£ç é¢„è§ˆ */}
                                <div className="col-span-1 bg-gray-900 text-blue-100 flex flex-col h-[400px] lg:h-auto">
                                    <div className="p-3 border-b border-gray-800 flex items-center justify-between">
                                        <span className="text-xs font-bold text-gray-400 uppercase tracking-wider">{t('proxy.multi_protocol.quick_integration')}</span>
                                        <div className="flex gap-2">
                                            {/* è¿™é‡Œå¯ä»¥æ”¾ cURL/Python åˆ‡æ¢ï¼Œæˆ–è€…ç›´æŽ¥é»˜è®¤æ˜¾ç¤º Pythonï¼Œæ ¹æ® selectedProtocol å†³å®š */}
                                            <span className="text-[10px] px-2 py-0.5 rounded bg-blue-500/20 text-blue-400 border border-blue-500/30">
                                                {selectedProtocol === 'anthropic' ? 'Python (Anthropic SDK)' : (selectedProtocol === 'gemini' ? 'Python (Google GenAI)' : 'Python (OpenAI SDK)')}
                                            </span>
                                        </div>
                                    </div>
                                    <div className="flex-1 relative overflow-hidden group">
                                        <div className="absolute inset-0 overflow-auto scrollbar-thin scrollbar-thumb-gray-700 scrollbar-track-transparent">
                                            <pre className="p-4 text-[10px] font-mono leading-relaxed">
                                                {getPythonExample(selectedModelId)}
                                            </pre>
                                        </div>
                                        <button
                                            onClick={() => copyToClipboard(getPythonExample(selectedModelId), 'example-code')}
                                            className="absolute top-4 right-4 p-2 bg-white/10 hover:bg-white/20 rounded-lg transition-colors text-white opacity-0 group-hover:opacity-100"
                                        >
                                            {copied === 'example-code' ? <CheckCircle size={16} /> : <Copy size={16} />}
                                        </button>
                                    </div>
                                    <div className="p-3 bg-gray-800/50 border-t border-gray-800 text-[10px] text-gray-400">
                                        {t('proxy.multi_protocol.click_tip')}
                                    </div>
                                </div>
                            </div>
                        </div>
                    )
                }
                {/* å„ç§å¯¹è¯æ¡† */}
                <ModalDialog
                    isOpen={isResetConfirmOpen}
                    title={t('proxy.dialog.reset_mapping_title') || 'é‡ç½®æ˜ å°„'}
                    message={t('proxy.dialog.reset_mapping_msg') || 'ç¡®å®šè¦é‡ç½®æ‰€æœ‰æ¨¡åž‹æ˜ å°„ä¸ºç³»ç»Ÿé»˜è®¤å—ï¼Ÿ'}
                    type="confirm"
                    isDestructive={true}
                    onConfirm={executeResetMapping}
                    onCancel={() => setIsResetConfirmOpen(false)}
                />

                <ModalDialog
                    isOpen={isRegenerateKeyConfirmOpen}
                    title={t('proxy.dialog.regenerate_key_title') || t('proxy.dialog.confirm_regenerate')}
                    message={t('proxy.dialog.regenerate_key_msg') || t('proxy.dialog.confirm_regenerate')}
                    type="confirm"
                    isDestructive={true}
                    onConfirm={executeGenerateApiKey}
                    onCancel={() => setIsRegenerateKeyConfirmOpen(false)}
                />

                <ModalDialog
                    isOpen={isClearBindingsConfirmOpen}
                    title={t('proxy.dialog.clear_bindings_title') || 'æ¸…é™¤ä¼šè¯ç»‘å®š'}
                    message={t('proxy.dialog.clear_bindings_msg') || 'ç¡®å®šè¦æ¸…é™¤æ‰€æœ‰ä¼šè¯ä¸Žè´¦å·çš„ç»‘å®šæ˜ å°„å—ï¼Ÿ'}
                    type="confirm"
                    isDestructive={true}
                    onConfirm={executeClearSessionBindings}
                    onCancel={() => setIsClearBindingsConfirmOpen(false)}
                />
            </div >
        </div>
    );
}

