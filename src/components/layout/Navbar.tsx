import { useState, useRef, useEffect } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { Sun, Moon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useConfigStore } from '../../stores/useConfigStore';

// Detect if running on Linux platform
const isLinux = navigator.userAgent.toLowerCase().includes('linux');

function Navbar() {
    const location = useLocation();
    const { t, i18n } = useTranslation();
    const { config, saveConfig } = useConfigStore();

    const navItems = [
        { path: '/', label: t('nav.dashboard') },
        { path: '/accounts', label: t('nav.accounts') },
        { path: '/api-proxy', label: t('nav.proxy') },
        { path: '/monitor', label: t('nav.call_records') },
        { path: '/settings', label: t('nav.settings') },
    ];

    const isActive = (path: string) => {
        if (path === '/') {
            return location.pathname === '/';
        }
        return location.pathname.startsWith(path);
    };

    const toggleTheme = async (event: React.MouseEvent<HTMLButtonElement>) => {
        if (!config) return;

        const newTheme = config.theme === 'light' ? 'dark' : 'light';

        // Use View Transition API if supported, but skip on Linux (may cause crash)
        if ('startViewTransition' in document && !isLinux) {
            const x = event.clientX;
            const y = event.clientY;
            const endRadius = Math.hypot(
                Math.max(x, window.innerWidth - x),
                Math.max(y, window.innerHeight - y)
            );

            // @ts-ignore
            const transition = document.startViewTransition(async () => {
                await saveConfig({
                    ...config,
                    theme: newTheme,
                    language: config.language
                });
            });

            transition.ready.then(() => {
                const isDarkMode = newTheme === 'dark';
                const clipPath = isDarkMode
                    ? [`circle(${endRadius}px at ${x}px ${y}px)`, `circle(0px at ${x}px ${y}px)`]
                    : [`circle(0px at ${x}px ${y}px)`, `circle(${endRadius}px at ${x}px ${y}px)`];

                document.documentElement.animate(
                    {
                        clipPath: clipPath
                    },
                    {
                        duration: 500,
                        easing: 'ease-in-out',
                        fill: 'forwards',
                        pseudoElement: isDarkMode ? '::view-transition-old(root)' : '::view-transition-new(root)'
                    }
                );
            });
        } else {
            // Fallback: direct switch (Linux or browsers without View Transition)
            await saveConfig({
                ...config,
                theme: newTheme,
                language: config.language
            });
        }
    };

    const [isLangOpen, setIsLangOpen] = useState(false);
    const langMenuRef = useRef<HTMLDivElement>(null);

    // Close language menu when clicking outside
    useEffect(() => {
        function handleClickOutside(event: MouseEvent) {
            if (langMenuRef.current && !langMenuRef.current.contains(event.target as Node)) {
                setIsLangOpen(false);
            }
        }
        document.addEventListener('mousedown', handleClickOutside);
        return () => {
            document.removeEventListener('mousedown', handleClickOutside);
        };
    }, []);

    const languages = [
        { code: 'zh', label: '简体中文', short: 'ZH' },
        { code: 'zh-TW', label: '繁體中文', short: 'TW' },
        { code: 'en', label: 'English', short: 'EN' },
        { code: 'ja', label: '日本語', short: 'JA' },
        { code: 'tr', label: 'Türkçe', short: 'TR' },
        { code: 'vi', label: 'Tiếng Việt', short: 'VI' },
        { code: 'pt', label: 'Português', short: 'PT' },
        { code: 'ko', label: '한국어', short: 'KO' },
        { code: 'ru', label: 'Русский', short: 'RU' },
    ];

    const handleLanguageChange = async (langCode: string) => {
        if (!config) return;

        await saveConfig({
            ...config,
            language: langCode,
            theme: config.theme
        });
        i18n.changeLanguage(langCode);
        setIsLangOpen(false);
    };

    return (
        <nav
            style={{ position: 'sticky', top: 0, zIndex: 50 }}
            className="pt-9 transition-all duration-200 bg-[#FAFBFC] dark:bg-base-300"
        >
            {/* 窗口拖拽区域 2 - 覆盖导航栏内容区域（在交互元素下方） */}
            <div
                className="absolute top-9 left-0 right-0 h-16"
                style={{ zIndex: 5, backgroundColor: 'rgba(0,0,0,0.001)' }}
                data-tauri-drag-region
            />

            <div className="max-w-7xl mx-auto px-8 relative" style={{ zIndex: 10 }}>
                <div className="flex items-center justify-between h-16">
                    {/* Logo - 左侧 */}
                    <div className="flex items-center">
                        <Link to="/" className="text-xl font-semibold text-gray-900 dark:text-base-content flex items-center gap-2">
                            Antigravity Manager Supreme
                        </Link>
                    </div>

                    {/* 药丸形状的导航标签 - 居中 */}
                    <div className="flex items-center gap-1 bg-gray-100 dark:bg-base-200 rounded-full p-1">
                        {navItems.map((item) => (
                            <Link
                                key={item.path}
                                to={item.path}
                                className={`px-6 py-2 rounded-full text-sm font-medium transition-all ${isActive(item.path)
                                    ? 'bg-gray-900 text-white shadow-sm dark:bg-white dark:text-gray-900'
                                    : 'text-gray-700 hover:text-gray-900 hover:bg-gray-200 dark:text-gray-400 dark:hover:text-base-content dark:hover:bg-base-100'
                                    }`}
                            >
                                {item.label}
                            </Link>
                        ))}
                    </div>

                    {/* 右侧快捷设置按钮 */}
                    <div className="flex items-center gap-2">
                        {/* 主题切换按钮 */}
                        <button
                            onClick={toggleTheme}
                            className="w-10 h-10 rounded-full bg-gray-100 dark:bg-base-200 hover:bg-gray-200 dark:hover:bg-base-100 flex items-center justify-center transition-colors"
                            title={config?.theme === 'light' ? t('nav.theme_to_dark') : t('nav.theme_to_light')}
                        >
                            {config?.theme === 'light' ? (
                                <Moon className="w-5 h-5 text-gray-700 dark:text-gray-300" />
                            ) : (
                                <Sun className="w-5 h-5 text-gray-700 dark:text-gray-300" />
                            )}
                        </button>

                        {/* 语言切换下拉菜单 */}
                        <div className="relative" ref={langMenuRef}>
                            <button
                                onClick={() => setIsLangOpen(!isLangOpen)}
                                className="w-10 h-10 rounded-full bg-gray-100 dark:bg-base-200 hover:bg-gray-200 dark:hover:bg-base-100 flex items-center justify-center transition-colors"
                                title={t('settings.general.language')}
                            >
                                <span className="text-sm font-bold text-gray-700 dark:text-gray-300">
                                    {languages.find(l => l.code === config?.language)?.short || 'EN'}
                                </span>
                            </button>

                            {/* 下拉菜单 */}
                            {isLangOpen && (
                                <div className="absolute right-0 mt-2 w-32 bg-white dark:bg-base-200 rounded-xl shadow-lg border border-gray-100 dark:border-base-100 py-1 overflow-hidden animate-in fade-in zoom-in-95 duration-200 origin-top-right">
                                    {languages.map((lang) => (
                                        <button
                                            key={lang.code}
                                            onClick={() => handleLanguageChange(lang.code)}
                                            className={`w-full px-4 py-2 text-left text-sm flex items-center justify-between hover:bg-gray-50 dark:hover:bg-base-100 transition-colors ${config?.language === lang.code
                                                ? 'text-blue-500 font-medium bg-blue-50 dark:bg-blue-900/10'
                                                : 'text-gray-700 dark:text-gray-300'
                                                }`}
                                        >
                                            <div className="flex items-center gap-3">
                                                <span className="font-mono font-bold w-6">{lang.short}</span>
                                                <span className="text-xs opacity-70">{lang.label}</span>
                                            </div>
                                            {config?.language === lang.code && (
                                                <span className="w-1.5 h-1.5 rounded-full bg-blue-500"></span>
                                            )}
                                        </button>
                                    ))}
                                </div>
                            )}
                        </div>
                    </div>
                </div>
            </div>
        </nav>
    );
}

export default Navbar;
