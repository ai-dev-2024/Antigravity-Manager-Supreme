import { useState } from 'react';
import { createPortal } from 'react-dom';
import { X, Upload, FileText, AlertCircle, CheckCircle2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ProxyEntry } from '../../../types/config';
import { generateUUID } from '../../../utils/uuid';

interface BatchImportModalProps {
    isOpen: boolean;
    onClose: () => void;
    onImport: (proxies: ProxyEntry[]) => void;
}

export default function BatchImportModal({ isOpen, onClose, onImport }: BatchImportModalProps) {
    const { t } = useTranslation();
    const [rawText, setRawText] = useState('');
    const [preview, setPreview] = useState<ProxyEntry[]>([]);
    const [error, setError] = useState<string | null>(null);

    if (!isOpen) return null;

    const parseProxies = (text: string) => {
        const lines = text.split('\n').filter(line => line.trim() !== '');
        const newProxies: ProxyEntry[] = [];
        const urlRegex = /([a-zA-Z0-9]+:\/\/[^\s]+)/; // Basic protocol://url matcher

        lines.forEach((line, index) => {
            try {
                const trimmedLine = line.trim();
                let url = '';
                // Strategy 1: Regex search for protocol://...
                const match = trimmedLine.match(urlRegex);
                if (match) {
                    url = match[0];
                } else {
                    // Check for host:port:user:pass or host:port
                    // logic: split by space first to get the "proxy part"
                    const firstWord = trimmedLine.split(/\s+/)[0];
                    const parts = firstWord.split(':');

                    if (parts.length === 4) {
                        // host:port:user:pass format
                        // Reconstruct to http://user:pass@host:port
                        const [host, port, user, pass] = parts;
                        url = `http://${user}:${pass}@${host}:${port}`;
                    } else if (parts.length === 2) {
                        // host:port format
                        const [host, port] = parts;
                        // Basic sanity check on port
                        if (!isNaN(Number(port))) {
                            url = `http://${host}:${port}`;
                        }
                    }
                }

                if (!url) {
                    // console.warn(`Line ${index + 1} skipped: no valid proxy found`);
                    return;
                }

                // Validation
                try {
                    new URL(url);
                } catch (e) {
                    console.warn(`Line ${index + 1} invalid URL: ${url}`);
                    return;
                }

                newProxies.push({
                    id: generateUUID(),
                    // Name will be assigned when adding to main list or just generic here
                    name: `Imported Proxy`,
                    url: url,
                    enabled: true,
                    priority: 1,
                    tags: ['imported'],
                    is_healthy: false,
                    latency: undefined
                });
            } catch (e) {
                console.error("Failed to parse line", line, e);
            }
        });

        // Fix names to be unique/sequential relative to this batch
        newProxies.forEach((p, i) => {
            p.name = `Proxy ${i + 1}`;
        });

        if (newProxies.length === 0 && lines.length > 0) {
            setError(t('settings.proxy_pool.no_valid_proxies', 'No valid proxies found'));
        } else {
            setError(null);
            setPreview(newProxies);
        }
    };

    const handleTextChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
        const text = e.target.value;
        setRawText(text);
        parseProxies(text);
    };

    const handleImport = () => {
        if (preview.length > 0) {
            onImport(preview);
            onClose();
            setRawText('');
            setPreview([]);
        }
    };

    return createPortal(
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4 animate-in fade-in duration-200">
            <div className="bg-white dark:bg-base-100 rounded-2xl shadow-xl w-full max-w-2xl max-h-[90vh] flex flex-col border border-gray-100 dark:border-base-300">
                <div className="flex items-center justify-between p-6 border-b border-gray-100 dark:border-base-200">
                    <h3 className="text-xl font-semibold text-gray-900 dark:text-base-content flex items-center gap-2">
                        <Upload size={20} className="text-blue-500" />
                        {t('settings.proxy_pool.import_title', 'Batch Import Proxies')}
                    </h3>
                    <button
                        onClick={onClose}
                        className="p-2 hover:bg-gray-100 dark:hover:bg-base-200 rounded-full transition-colors text-gray-500"
                    >
                        <X size={20} />
                    </button>
                </div>

                <div className="flex-1 overflow-y-auto p-6 space-y-6">
                    <div>
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                            {t('settings.proxy_pool.import_label', 'Paste Proxy List (One per line)')}
                        </label>
                        <div className="text-xs text-gray-500 mb-2">
                            {t('settings.proxy_pool.import_hint', 'Supported formats: protocol://user:pass@host:port, host:port:user:pass')}
                        </div>
                        <textarea
                            className="w-full h-40 px-4 py-3 border border-gray-200 dark:border-base-300 rounded-xl focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent bg-gray-50 dark:bg-base-200 text-gray-900 dark:text-base-content font-mono text-sm resize-none"
                            placeholder="http://user:pass@127.0.0.1:8080&#10;127.0.0.1:8080:user:pass"
                            value={rawText}
                            onChange={handleTextChange}
                        />
                    </div>

                    {error && (
                        <div className="p-4 bg-red-50 dark:bg-red-900/10 rounded-xl flex items-start gap-3 text-red-600 dark:text-red-400">
                            <AlertCircle size={18} className="mt-0.5 shrink-0" />
                            <p className="text-sm">{error}</p>
                        </div>
                    )}

                    {preview.length > 0 && (
                        <div>
                            <h4 className="text-sm font-medium text-gray-900 dark:text-base-content mb-3 flex items-center gap-2">
                                <FileText size={16} />
                                {t('settings.proxy_pool.import_preview', 'Preview')}
                                <span className="px-2 py-0.5 rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 text-xs">
                                    {preview.length} valid
                                </span>
                            </h4>
                            <div className="bg-gray-50 dark:bg-base-200 rounded-xl border border-gray-200 dark:border-base-300 max-h-40 overflow-y-auto">
                                <table className="w-full text-sm">
                                    <thead className="bg-gray-100 dark:bg-base-300 sticky top-0">
                                        <tr>
                                            <th className="px-4 py-2 text-left font-medium text-gray-600 dark:text-gray-400 w-12">#</th>
                                            {/* Removed Name column from preview since it's generic now, or keep it? user said "simpler naming". Keeping it simple. */}
                                            <th className="px-4 py-2 text-left font-medium text-gray-600 dark:text-gray-400">URL</th>
                                        </tr>
                                    </thead>
                                    <tbody className="divide-y divide-gray-200 dark:divide-base-300">
                                        {preview.map((proxy, idx) => (
                                            <tr key={idx} className="hover:bg-gray-100 dark:hover:bg-base-300/50">
                                                <td className="px-4 py-2 text-gray-500">{idx + 1}</td>
                                                <td className="px-4 py-2 text-gray-900 dark:text-base-content font-mono truncate max-w-[300px]" title={proxy.url}>
                                                    {proxy.url}
                                                </td>
                                            </tr>
                                        ))}
                                    </tbody>
                                </table>
                            </div>
                        </div>
                    )}
                </div>

                <div className="p-6 border-t border-gray-100 dark:border-base-200 flex justify-end gap-3 bg-gray-50 dark:bg-base-200/50 rounded-b-2xl">
                    <button
                        onClick={onClose}
                        className="px-5 py-2.5 rounded-xl border border-gray-200 dark:border-base-300 text-gray-700 dark:text-gray-300 font-medium hover:bg-gray-100 dark:hover:bg-base-200 transition-colors"
                    >
                        {t('common.cancel', 'Cancel')}
                    </button>
                    <button
                        onClick={handleImport}
                        disabled={preview.length === 0}
                        className="px-5 py-2.5 rounded-xl bg-blue-500 hover:bg-blue-600 active:scale-95 text-white font-medium shadow-sm shadow-blue-200 dark:shadow-none transition-all disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                    >
                        <CheckCircle2 size={18} />
                        {t('settings.proxy_pool.import_confirm', 'Import {{count}} Proxies', { count: preview.length })}
                    </button>
                </div>
            </div>
        </div>,
        document.body
    );
}
