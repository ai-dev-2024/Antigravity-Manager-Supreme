import { useState, useEffect } from 'react';
import { Save, Github, ExternalLink, Sparkles, RefreshCw, Download } from 'lucide-react';
import { request as invoke } from '../utils/request';
import { open } from '@tauri-apps/plugin-dialog';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { getVersion } from '@tauri-apps/api/app';
import { useConfigStore } from '../stores/useConfigStore';
import { AppConfig } from '../types/config';
import ModalDialog from '../components/common/ModalDialog';
import { showToast } from '../components/common/ToastContainer';

import { useTranslation } from 'react-i18next';

function Settings() {
    const { t } = useTranslation();
    const { config, loadConfig, saveConfig } = useConfigStore();
    const [activeTab, setActiveTab] = useState<'general' | 'account' | 'proxy' | 'advanced' | 'about'>('general');
    const [formData, setFormData] = useState<AppConfig>({
        language: 'zh',
        theme: 'system',
        auto_refresh: false,
        refresh_interval: 15,
        auto_sync: false,
        sync_interval: 5,
        scheduled_warmup: {
            enabled: false,
            monitored_models: []
        },
        quota_protection: {
            enabled: false,
            threshold_percentage: 10,
            monitored_models: []
        },
        pinned_quota_models: {
            models: []
        },
        proxy: {
            enabled: false,
            port: 8080,
            api_key: '',
            auto_start: false,
            request_timeout: 120,
            enable_logging: false,
            upstream_proxy: {
                enabled: false,
                url: ''
            }
        }
    });

    // Dialog state
    const [isClearLogsOpen, setIsClearLogsOpen] = useState(false);
    const [dataDirPath, setDataDirPath] = useState<string>('~/.antigravity_tools/');

    // Update state
    const [isCheckingUpdate, setIsCheckingUpdate] = useState(false);
    const [updateAvailable, setUpdateAvailable] = useState<{ version: string; notes: string } | null>(null);
    const [isDownloading, setIsDownloading] = useState(false);
    const [downloadProgress, setDownloadProgress] = useState(0);
    const [appVersion, setAppVersion] = useState<string>('...');

    // Check for updates handler
    const handleCheckForUpdates = async () => {
        setIsCheckingUpdate(true);
        setUpdateAvailable(null);
        try {
            const update = await check();
            if (update) {
                setUpdateAvailable({
                    version: update.version,
                    notes: update.body || 'No release notes available'
                });
                showToast(`New version ${update.version} available!`, 'success');
            } else {
                showToast('You are on the latest version!', 'info');
            }
        } catch (error) {
            console.error('Failed to check for updates:', error);
            const errorMsg = error instanceof Error ? error.message : String(error);
            if (errorMsg.includes('No update available')) {
                showToast('You are on the latest version!', 'info');
            } else {
                showToast(`Update check failed: ${errorMsg}`, 'error');
            }
        } finally {
            setIsCheckingUpdate(false);
        }
    };

    // Download and install update
    const handleDownloadUpdate = async () => {
        if (!updateAvailable) return;
        setIsDownloading(true);
        setDownloadProgress(0);
        try {
            const update = await check();
            if (update) {
                let downloaded = 0;
                let contentLength = 0;
                await update.downloadAndInstall((event) => {
                    if (event.event === 'Started') {
                        contentLength = event.data.contentLength || 0;
                    } else if (event.event === 'Progress') {
                        downloaded += event.data.chunkLength;
                        if (contentLength > 0) {
                            setDownloadProgress(Math.round((downloaded / contentLength) * 100));
                        }
                    } else if (event.event === 'Finished') {
                        setDownloadProgress(100);
                    }
                });
                showToast('Update downloaded! Restarting...', 'success');
                await relaunch();
            }
        } catch (error) {
            console.error('Failed to download update:', error);
            showToast('Failed to download update', 'error');
        } finally {
            setIsDownloading(false);
        }
    };

    useEffect(() => {
        loadConfig();

        // Get real data directory path
        invoke<string>('get_data_dir_path')
            .then(path => setDataDirPath(path))
            .catch(err => console.error('Failed to get data dir:', err));

        // Load app version dynamically
        getVersion()
            .then(version => setAppVersion(version))
            .catch(err => console.error('Failed to get version:', err));
    }, [loadConfig]);

    useEffect(() => {
        if (config) {
            setFormData(config);
        }
    }, [config]);

    const handleSave = async () => {
        try {
            // 校验：如果启用了上游代理但没有填写地址，给出提示
            const proxyEnabled = formData.proxy?.upstream_proxy?.enabled;
            const proxyUrl = formData.proxy?.upstream_proxy?.url?.trim();
            if (proxyEnabled && !proxyUrl) {
                showToast(t('proxy.config.upstream_proxy.validation_error', '启用上游代理时必须填写代理地址'), 'error');
                return;
            }

            // 强制开启后台自动刷新，确保联动逻辑生效
            await saveConfig({ ...formData, auto_refresh: true });
            showToast(t('common.saved'), 'success');

            // 如果修改了代理配置，提示用户需要重启
            if (proxyEnabled && proxyUrl) {
                showToast(t('proxy.config.upstream_proxy.restart_hint', '代理配置已保存，重启应用后生效'), 'info');
            }
        } catch (error) {
            showToast(`${t('common.error')}: ${error}`, 'error');
        }
    };

    const confirmClearLogs = async () => {
        try {
            await invoke('clear_log_cache');
            showToast(t('settings.advanced.logs_cleared'), 'success');
        } catch (error) {
            showToast(`${t('common.error')}: ${error}`, 'error');
        }
        setIsClearLogsOpen(false);
    };

    const handleOpenDataDir = async () => {
        try {
            await invoke('open_data_folder');
        } catch (error) {
            showToast(`${t('common.error')}: ${error}`, 'error');
        }
    };

    const handleSelectExportPath = async () => {
        try {
            // @ts-ignore
            const selected = await open({
                directory: true,
                multiple: false,
                title: t('settings.advanced.export_path'),
            });
            if (selected && typeof selected === 'string') {
                setFormData({ ...formData, default_export_path: selected });
            }
        } catch (error) {
            showToast(`${t('common.error')}: ${error}`, 'error');
        }
    };

    const handleSelectAntigravityPath = async () => {
        try {
            const selected = await open({
                directory: false,
                multiple: false,
                title: t('settings.advanced.antigravity_path_select'),
            });
            if (selected && typeof selected === 'string') {
                setFormData({ ...formData, antigravity_executable: selected });
            }
        } catch (error) {
            showToast(`${t('common.error')}: ${error}`, 'error');
        }
    };


    const handleDetectAntigravityPath = async () => {
        try {
            const path = await invoke<string>('get_antigravity_path', { bypassConfig: true });
            setFormData({ ...formData, antigravity_executable: path });
            showToast(t('settings.advanced.antigravity_path_detected'), 'success');
        } catch (error) {
            showToast(`${t('common.error')}: ${error}`, 'error');
        }
    };

    return (
        <div className="h-full w-full overflow-y-auto">
            <div className="p-5 space-y-4 max-w-7xl mx-auto">
                {/* Top toolbar: Tab navigation and save button */}
                <div className="flex justify-between items-center">
                    {/* Tab navigation - Top navbar style: outer gray container */}
                    <div className="flex items-center gap-1 bg-gray-100 dark:bg-base-200 rounded-full p-1 w-fit">
                        <button
                            className={`px-6 py-2 rounded-full text-sm font-medium transition-all ${activeTab === 'general'
                                ? 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'
                                }`}
                            onClick={() => setActiveTab('general')}
                        >
                            {t('settings.tabs.general')}
                        </button>
                        <button
                            className={`px-6 py-2 rounded-full text-sm font-medium transition-all ${activeTab === 'account'
                                ? 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'
                                }`}
                            onClick={() => setActiveTab('account')}
                        >
                            {t('settings.tabs.account')}
                        </button>
                        <button
                            className={`px-6 py-2 rounded-full text-sm font-medium transition-all ${activeTab === 'proxy'
                                ? 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'
                                }`}
                            onClick={() => setActiveTab('proxy')}
                        >
                            {t('settings.tabs.proxy')}
                        </button>
                        <button
                            className={`px-6 py-2 rounded-full text-sm font-medium transition-all ${activeTab === 'advanced'
                                ? 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'
                                }`}
                            onClick={() => setActiveTab('advanced')}
                        >
                            {t('settings.tabs.advanced')}
                        </button>
                        <button
                            className={`px-6 py-2 rounded-full text-sm font-medium transition-all ${activeTab === 'about'
                                ? 'bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200'
                                }`}
                            onClick={() => setActiveTab('about')}
                        >
                            {t('settings.tabs.about')}
                        </button>
                    </div>

                    <button
                        className="px-4 py-2 bg-primary text-primary-foreground text-sm rounded-lg hover:bg-primary/90 transition-colors flex items-center gap-2 shadow-sm"
                        onClick={handleSave}
                    >
                        <Save className="w-4 h-4" />
                        {t('settings.save')}
                    </button>
                </div>

                {/* Settings Form */}
                <div className="bg-card rounded-2xl p-6 shadow-sm border border-border">
                    {/* General Settings */}
                    {activeTab === 'general' && (
                        <div className="space-y-6">
                            <h2 className="text-lg font-semibold text-card-foreground">{t('settings.general.title')}</h2>

                            {/* Language Selection */}
                            <div>
                                <label className="block text-sm font-medium text-card-foreground mb-2">{t('settings.general.language')}</label>
                                <select
                                    className="w-full px-4 py-4 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-card-foreground bg-muted notranslate"
                                    value={formData.language}
                                    onChange={(e) => {
                                        const langCode = e.target.value;
                                        setFormData({ ...formData, language: langCode });
                                        // Trigger Google Translate
                                        if ((window as any).changeGoogleTranslateLanguage) {
                                            (window as any).changeGoogleTranslateLanguage(langCode);
                                        }
                                    }}
                                >
                                    <option value="en">English</option>
                                    <option value="es">Español</option>
                                    <option value="fr">Français</option>
                                    <option value="de">Deutsch</option>
                                    <option value="it">Italiano</option>
                                    <option value="pt">Português</option>
                                    <option value="ko">한국어</option>
                                    <option value="ru">Русский</option>
                                    <option value="ja">日本語</option>
                                    <option value="ko">한국어</option>
                                    <option value="zh">简体中文</option>
                                    <option value="zh-TW">繁體中文</option>
                                    <option value="ar">العربية</option>
                                    <option value="hi">हिन्दी</option>
                                    <option value="bn">বাংলা</option>
                                    <option value="tr">Türkçe</option>
                                    <option value="vi">Tiếng Việt</option>
                                    <option value="th">ไทย</option>
                                    <option value="nl">Nederlands</option>
                                    <option value="pl">Polski</option>
                                    <option value="uk">Українська</option>
                                </select>
                            </div>

                            {/* Theme Selection */}
                            <div>
                                <label className="block text-sm font-medium text-card-foreground mb-2">{t('settings.general.theme')}</label>
                                <select
                                    className="w-full px-4 py-4 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-card-foreground bg-muted"
                                    value={formData.theme}
                                    onChange={async (e) => {
                                        const newTheme = e.target.value;
                                        setFormData({ ...formData, theme: newTheme });

                                        // Apply theme directly to DOM first
                                        const root = document.documentElement;
                                        const resolvedTheme = newTheme === 'system'
                                            ? (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
                                            : newTheme;

                                        root.setAttribute('data-theme', resolvedTheme);
                                        root.style.backgroundColor = resolvedTheme === 'dark' ? '#1d232a' : '#FAFBFC';
                                        if (resolvedTheme === 'dark') {
                                            root.classList.add('dark');
                                        } else {
                                            root.classList.remove('dark');
                                        }
                                        localStorage.setItem('app-theme-preference', newTheme);

                                        // Save to config in background
                                        if (config) {
                                            saveConfig({ ...config, theme: newTheme }).catch(console.error);
                                        }
                                    }}
                                >
                                    <option value="light">{t('settings.general.theme_light')}</option>
                                    <option value="dark">{t('settings.general.theme_dark')}</option>
                                    <option value="system">{t('settings.general.theme_system')}</option>
                                </select>
                            </div>

                            {/* Auto-start on boot */}
                            <div>
                                <label className="block text-sm font-medium text-card-foreground mb-2">{t('settings.general.auto_launch')}</label>
                                <select
                                    className="w-full px-4 py-4 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-card-foreground bg-muted"
                                    value={formData.auto_launch ? 'enabled' : 'disabled'}
                                    onChange={async (e) => {
                                        const enabled = e.target.value === 'enabled';
                                        try {
                                            await invoke('toggle_auto_launch', { enable: enabled });
                                            setFormData({ ...formData, auto_launch: enabled });
                                            showToast(enabled ? 'Auto-launch enabled' : 'Auto-launch disabled', 'success');
                                        } catch (error) {
                                            showToast(`${t('common.error')}: ${error}`, 'error');
                                        }
                                    }}
                                >
                                    <option value="disabled">{t('settings.general.auto_launch_disabled')}</option>
                                    <option value="enabled">{t('settings.general.auto_launch_enabled')}</option>
                                </select>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mt-2">{t('settings.general.auto_launch_desc')}</p>
                            </div>
                        </div>
                    )}

                    {/* Account settings */}
                    {activeTab === 'account' && (
                        <div className="space-y-6">
                            <h2 className="text-lg font-semibold text-card-foreground">{t('settings.account.title')}</h2>

                            {/* Auto-refresh quota */}
                            <div className="flex items-center justify-between p-4 bg-muted rounded-lg border border-gray-100 dark:border-base-300">
                                <div>
                                    <div className="font-medium text-card-foreground">{t('settings.account.auto_refresh')}</div>
                                    <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">{t('settings.account.auto_refresh_desc')}</p>
                                </div>
                                <label className="relative inline-flex items-center cursor-pointer">
                                    <input
                                        type="checkbox"
                                        className="sr-only peer"
                                        checked={formData.auto_refresh}
                                        onChange={(e) => setFormData({ ...formData, auto_refresh: e.target.checked })}
                                    />
                                    <div className="w-11 h-6 bg-gray-200 dark:bg-base-300 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-500"></div>
                                </label>
                            </div>

                            {/* Refresh interval */}
                            {formData.auto_refresh && (
                                <div className="ml-4">
                                    <label className="block text-sm font-medium text-card-foreground mb-2">{t('settings.account.refresh_interval')}</label>
                                    <input
                                        type="number"
                                        className="w-32 px-4 py-4 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-card-foreground bg-muted"
                                        min="1"
                                        max="60"
                                        value={formData.refresh_interval}
                                        onChange={(e) => setFormData({ ...formData, refresh_interval: parseInt(e.target.value) })}
                                    />
                                </div>
                            )}

                            {/* Auto-sync current account */}
                            <div className="flex items-center justify-between p-4 bg-muted rounded-lg border border-gray-100 dark:border-base-300">
                                <div>
                                    <div className="font-medium text-card-foreground">{t('settings.account.auto_sync')}</div>
                                    <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">{t('settings.account.auto_sync_desc')}</p>
                                </div>
                                <label className="relative inline-flex items-center cursor-pointer">
                                    <input
                                        type="checkbox"
                                        className="sr-only peer"
                                        checked={formData.auto_sync}
                                        onChange={(e) => setFormData({ ...formData, auto_sync: e.target.checked })}
                                    />
                                    <div className="w-11 h-6 bg-gray-200 dark:bg-base-300 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-500"></div>
                                </label>
                            </div>

                            {/* Sync interval */}
                            {formData.auto_sync && (
                                <div className="ml-4">
                                    <label className="block text-sm font-medium text-card-foreground mb-2">{t('settings.account.sync_interval')}</label>
                                    <input
                                        type="number"
                                        className="w-32 px-4 py-4 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-card-foreground bg-muted"
                                        min="1"
                                        max="60"
                                        value={formData.sync_interval}
                                        onChange={(e) => setFormData({ ...formData, sync_interval: parseInt(e.target.value) })}
                                    />
                                </div>
                            )}
                        </div>
                    )}

                    {/* Advanced settings */}
                    {activeTab === 'advanced' && (
                        <div className="space-y-4">
                            <h2 className="text-lg font-semibold text-card-foreground">{t('settings.advanced.title')}</h2>

                            {/* Default export path */}
                            <div>
                                <label className="block text-sm font-medium text-card-foreground mb-1">{t('settings.advanced.export_path')}</label>
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        className="flex-1 px-4 py-4 border border-border rounded-lg bg-muted text-card-foreground font-medium"
                                        value={formData.default_export_path || t('settings.advanced.export_path_placeholder')}
                                        readOnly
                                    />
                                    {formData.default_export_path && (
                                        <button
                                            className="px-4 py-2 border border-border text-red-600 dark:text-red-400 rounded-lg hover:bg-red-50 dark:hover:bg-red-900/10 transition-colors"
                                            onClick={() => setFormData({ ...formData, default_export_path: undefined })}
                                        >
                                            {t('common.clear')}
                                        </button>
                                    )}
                                    <button
                                        className="px-4 py-2 border border-border text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-base-200 hover:text-gray-900 dark:hover:text-base-content transition-colors"
                                        onClick={handleSelectExportPath}
                                    >
                                        {t('settings.advanced.select_btn')}
                                    </button>
                                </div>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mt-2">{t('settings.advanced.default_export_path_desc')}</p>
                            </div>

                            {/* Data directory */}
                            <div>
                                <label className="block text-sm font-medium text-card-foreground mb-1">{t('settings.advanced.data_dir')}</label>
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        className="flex-1 px-4 py-4 border border-border rounded-lg bg-muted text-card-foreground font-medium"
                                        value={dataDirPath}
                                        readOnly
                                    />
                                    <button
                                        className="px-4 py-2 border border-border text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-base-200 hover:text-gray-900 dark:hover:text-base-content transition-colors"
                                        onClick={handleOpenDataDir}
                                    >
                                        {t('settings.advanced.open_btn')}
                                    </button>
                                </div>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mt-2">{t('settings.advanced.data_dir_desc')}</p>
                            </div>

                            {/* Antigravity executable path */}
                            <div>
                                <label className="block text-sm font-medium text-card-foreground mb-1">
                                    {t('settings.advanced.antigravity_path')}
                                </label>
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        className="flex-1 px-4 py-4 border border-border rounded-lg bg-muted text-card-foreground font-medium"
                                        value={formData.antigravity_executable || ''}
                                        placeholder={t('settings.advanced.antigravity_path_placeholder')}
                                        onChange={(e) => setFormData({ ...formData, antigravity_executable: e.target.value })}
                                    />
                                    {formData.antigravity_executable && (
                                        <button
                                            className="px-4 py-2 border border-border text-red-600 dark:text-red-400 rounded-lg hover:bg-red-50 dark:hover:bg-red-900/10 transition-colors"
                                            onClick={() => setFormData({ ...formData, antigravity_executable: undefined })}
                                        >
                                            {t('common.clear')}
                                        </button>
                                    )}
                                    <button
                                        className="px-4 py-2 border border-border text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-base-200 transition-colors"
                                        onClick={handleDetectAntigravityPath}
                                    >
                                        {t('settings.advanced.detect_btn')}
                                    </button>
                                    <button
                                        className="px-4 py-2 border border-border text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-base-200 transition-colors"
                                        onClick={handleSelectAntigravityPath}
                                    >
                                        {t('settings.advanced.select_btn')}
                                    </button>
                                </div>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mt-2">
                                    {t('settings.advanced.antigravity_path_desc')}
                                </p>
                            </div>

                            {/* Antigravity startup arguments */}
                            <div>
                                <label className="block text-sm font-medium text-card-foreground mb-1">
                                    {t('settings.advanced.antigravity_args')}
                                </label>
                                <div className="flex gap-2">
                                    <input
                                        type="text"
                                        className="flex-1 px-4 py-4 border border-border rounded-lg bg-muted text-card-foreground font-medium"
                                        value={formData.antigravity_args ? formData.antigravity_args.join(' ') : ''}
                                        placeholder={t('settings.advanced.antigravity_args_placeholder')}
                                        onChange={(e) => {
                                            const args = e.target.value.trim() === '' ? [] : e.target.value.split(' ').map(arg => arg.trim()).filter(arg => arg !== '');
                                            setFormData({ ...formData, antigravity_args: args });
                                        }}
                                    />
                                    <button
                                        className="px-4 py-2 border border-border text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-base-200 transition-colors"
                                        onClick={async () => {
                                            try {
                                                const args = await invoke<string[]>('get_antigravity_args');
                                                setFormData({ ...formData, antigravity_args: args });
                                                showToast(t('settings.advanced.antigravity_args_detected'), 'success');
                                            } catch (error) {
                                                showToast(`${t('settings.advanced.antigravity_args_detect_error')}: ${error}`, 'error');
                                            }
                                        }}
                                    >
                                        {t('settings.advanced.detect_args_btn')}
                                    </button>
                                </div>
                                <p className="text-sm text-gray-500 dark:text-gray-400 mt-2">
                                    {t('settings.advanced.antigravity_args_desc')}
                                </p>
                            </div>

                            <div className="border-t border-gray-200 dark:border-base-200 pt-4">
                                <h3 className="font-medium text-card-foreground mb-3">{t('settings.advanced.logs_title')}</h3>
                                <div className="bg-muted border border-border rounded-lg p-3 mb-3">
                                    <p className="text-sm text-gray-600 dark:text-gray-400">{t('settings.advanced.logs_desc')}</p>
                                </div>
                                <div className="badge badge-primary badge-outline gap-2 font-mono">
                                    v3.3.50
                                </div>
                                <div className="flex items-center gap-4">
                                    <button
                                        className="px-4 py-2 border border-border text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-100 dark:hover:bg-base-200 transition-colors"
                                        onClick={() => setIsClearLogsOpen(true)}
                                    >
                                        {t('settings.advanced.clear_logs')}
                                    </button>
                                </div>
                            </div>
                        </div>
                    )}

                    {/* Proxy settings */}
                    {activeTab === 'proxy' && (
                        <div className="space-y-6">
                            <h2 className="text-lg font-semibold text-card-foreground">{t('settings.proxy.title')}</h2>

                            <div className="p-4 bg-muted rounded-lg border border-gray-100 dark:border-base-300">
                                <h3 className="text-md font-semibold text-card-foreground mb-3 flex items-center gap-2">
                                    <Sparkles size={18} className="text-blue-500" />
                                    {t('proxy.config.upstream_proxy.title')}
                                </h3>
                                <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
                                    {t('proxy.config.upstream_proxy.desc')}
                                </p>

                                <div className="space-y-4">
                                    <div className="flex items-center">
                                        <label className="flex items-center cursor-pointer gap-3">
                                            <div className="relative">
                                                <input
                                                    type="checkbox"
                                                    className="sr-only"
                                                    checked={formData.proxy?.upstream_proxy?.enabled || false}
                                                    onChange={(e) => setFormData({
                                                        ...formData,
                                                        proxy: {
                                                            ...formData.proxy,
                                                            upstream_proxy: {
                                                                ...formData.proxy.upstream_proxy,
                                                                enabled: e.target.checked
                                                            }
                                                        }
                                                    })}
                                                />
                                                <div className={`block w-14 h-8 rounded-full transition-colors ${formData.proxy?.upstream_proxy?.enabled ? 'bg-blue-500' : 'bg-gray-300 dark:bg-base-300'}`}></div>
                                                <div className={`dot absolute left-1 top-1 bg-white w-6 h-6 rounded-full transition-transform ${formData.proxy?.upstream_proxy?.enabled ? 'transform translate-x-6' : ''}`}></div>
                                            </div>
                                            <span className="text-sm font-medium text-card-foreground">
                                                {t('proxy.config.upstream_proxy.enable')}
                                            </span>
                                        </label>
                                    </div>

                                    <div>
                                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                            {t('proxy.config.upstream_proxy.url')}
                                        </label>
                                        <input
                                            type="text"
                                            value={formData.proxy?.upstream_proxy?.url || ''}
                                            onChange={(e) => setFormData({
                                                ...formData,
                                                proxy: {
                                                    ...formData.proxy,
                                                    upstream_proxy: {
                                                        ...formData.proxy.upstream_proxy,
                                                        url: e.target.value
                                                    }
                                                }
                                            })}
                                            placeholder={t('proxy.config.upstream_proxy.url_placeholder')}
                                            className="w-full px-4 py-4 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent text-card-foreground bg-muted"
                                        />
                                        <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                                            {t('proxy.config.upstream_proxy.tip')}
                                        </p>
                                    </div>
                                </div>
                            </div>
                        </div>
                    )}
                    {activeTab === 'about' && (
                        <div className="flex flex-col h-full animate-in fade-in duration-500">
                            <div className="flex-1 flex flex-col justify-center items-center space-y-8">
                                {/* Branding Section */}
                                <div className="text-center space-y-4">
                                    <div className="relative inline-block group">
                                        <div className="absolute inset-0 bg-blue-500/20 rounded-3xl blur-xl group-hover:blur-2xl transition-all duration-500"></div>
                                        <img
                                            src="/icon.png"
                                            alt="Antigravity Logo"
                                            className="relative w-24 h-24 rounded-3xl shadow-2xl transform group-hover:scale-105 transition-all duration-500 rotate-3 group-hover:rotate-6 object-cover"
                                        />
                                    </div>

                                    <div>
                                        <h3 className="text-3xl font-black text-card-foreground tracking-tight mb-2">Antigravity Manager Supreme</h3>
                                        <div className="flex items-center justify-center gap-2 text-sm">
                                            <span className="px-2.5 py-0.5 rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 font-medium border border-blue-200 dark:border-blue-800">
                                                v3.3.50
                                            </span>
                                            <span className="text-gray-400 dark:text-gray-600">•</span>
                                            <span className="text-gray-500 dark:text-gray-400">Professional Account Management</span>
                                        </div>
                                    </div>
                                </div>

                                {/* GitHub Card - Centered */}
                                <div className="flex justify-center w-full max-w-md px-4">
                                    <a
                                        href="https://github.com/ai-dev-2024/Antigravity-Manager-Supreme"
                                        target="_blank"
                                        rel="noreferrer"
                                        className="bg-card p-4 rounded-2xl border border-gray-100 dark:border-base-300 shadow-sm hover:shadow-md hover:border-gray-300 dark:hover:border-gray-600 transition-all group flex flex-col items-center text-center gap-3 cursor-pointer w-full"
                                    >
                                        <div className="p-3 bg-gray-50 dark:bg-gray-800 rounded-xl group-hover:scale-110 transition-transform duration-300">
                                            <Github className="w-6 h-6 text-gray-900 dark:text-white" />
                                        </div>
                                        <div>
                                            <div className="text-xs text-gray-400 uppercase tracking-wider font-semibold mb-1">{t('settings.about.github')}</div>
                                            <div className="flex items-center gap-1 font-bold text-card-foreground">
                                                <span>{t('settings.about.view_code')}</span>
                                                <ExternalLink className="w-3 h-3 text-gray-400" />
                                            </div>
                                        </div>
                                    </a>
                                </div>

                                {/* Check for Updates */}
                                <div className="flex flex-col items-center gap-3 w-full max-w-md px-4">
                                    {!updateAvailable ? (
                                        <button
                                            onClick={handleCheckForUpdates}
                                            disabled={isCheckingUpdate}
                                            className="flex items-center gap-2 px-4 py-2 bg-blue-500 hover:bg-blue-600 text-white rounded-lg font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
                                        >
                                            <RefreshCw className={`w-4 h-4 ${isCheckingUpdate ? 'animate-spin' : ''}`} />
                                            {isCheckingUpdate ? 'Checking...' : 'Check for Updates'}
                                        </button>
                                    ) : (
                                        <div className="flex flex-col items-center gap-2 w-full">
                                            <div className="text-sm text-green-600 dark:text-green-400 font-medium">
                                                🎉 Version {updateAvailable.version} available!
                                            </div>
                                            {isDownloading ? (
                                                <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
                                                    <div
                                                        className="bg-blue-500 h-2 rounded-full transition-all duration-300"
                                                        style={{ width: `${downloadProgress}%` }}
                                                    />
                                                </div>
                                            ) : (
                                                <button
                                                    onClick={handleDownloadUpdate}
                                                    className="flex items-center gap-2 px-4 py-2 bg-green-500 hover:bg-green-600 text-white rounded-lg font-medium transition-colors"
                                                >
                                                    <Download className="w-4 h-4" />
                                                    Download & Install
                                                </button>
                                            )}
                                        </div>
                                    )}
                                </div>

                                {/* Tech Stack Badges */}
                                <div className="flex gap-2 justify-center">
                                    <div className="px-3 py-1 bg-muted rounded-lg text-xs font-medium text-gray-500 dark:text-gray-400 border border-gray-100 dark:border-base-300">
                                        Tauri v2
                                    </div>
                                    <div className="px-3 py-1 bg-muted rounded-lg text-xs font-medium text-gray-500 dark:text-gray-400 border border-gray-100 dark:border-base-300">
                                        React 19
                                    </div>
                                    <div className="px-3 py-1 bg-muted rounded-lg text-xs font-medium text-gray-500 dark:text-gray-400 border border-gray-100 dark:border-base-300">
                                        TypeScript
                                    </div>
                                </div>
                            </div>
                        </div>
                    )}
                </div>

                <ModalDialog
                    isOpen={isClearLogsOpen}
                    title={t('settings.advanced.clear_logs_title')}
                    message={t('settings.advanced.clear_logs_msg')}
                    type="confirm"
                    confirmText={t('common.clear')}
                    cancelText={t('common.cancel')}
                    isDestructive={true}
                    onConfirm={confirmClearLogs}
                    onCancel={() => setIsClearLogsOpen(false)}
                />
            </div>
        </div>
    );
}

export default Settings;
