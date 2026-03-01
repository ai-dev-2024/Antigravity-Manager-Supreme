
import React, { useState } from 'react';
import { Edit2, Trash2, Power, Globe } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ProxyEntry } from '../../../types/config';
import ProxyEditModal from './ProxyEditModal';
import { Account } from '../../../types/account';

interface ProxyListProps {
    proxies: ProxyEntry[];
    onUpdate: (proxies: ProxyEntry[]) => void;
    accountBindings: Record<string, string>;
    accounts: Account[];
    selectedIds: Set<string>;
    onSelectionChange: (ids: Set<string>) => void;
    isTesting?: boolean;
}

export default function ProxyList({ proxies, onUpdate, accountBindings, accounts, selectedIds, onSelectionChange, isTesting }: ProxyListProps) {
    const { t } = useTranslation();
    const [editingProxy, setEditingProxy] = useState<ProxyEntry | undefined>(undefined);
    const [isEditModalOpen, setIsEditModalOpen] = useState(false);

    const handleEdit = (proxy: ProxyEntry) => {
        setEditingProxy(proxy);
        setIsEditModalOpen(true);
    };

    const handleDelete = (id: string) => {
        if (confirm(t('settings.proxy_pool.confirm_delete', 'Are you sure you want to delete this proxy?'))) {
            onUpdate(proxies.filter(p => p.id !== id));
        }
    };

    const handleSaveProxy = (entry: ProxyEntry) => {
        if (editingProxy) {
            onUpdate(proxies.map(p => p.id === entry.id ? entry : p));
        }
        setEditingProxy(undefined);
    };

    const handleToggleEnabled = (id: string) => {
        onUpdate(proxies.map(p => p.id === id ? { ...p, enabled: !p.enabled } : p));
    };

    const sortedProxies = [...proxies].sort((a, b) => a.priority - b.priority);

    const handleSelectAll = (e: React.ChangeEvent<HTMLInputElement>) => {
        if (e.target.checked) {
            onSelectionChange(new Set(proxies.map(p => p.id)));
        } else {
            onSelectionChange(new Set());
        }
    };

    const handleSelectOne = (id: string) => {
        const newSelected = new Set(selectedIds);
        if (newSelected.has(id)) {
            newSelected.delete(id);
        } else {
            newSelected.add(id);
        }
        onSelectionChange(newSelected);
    };

    const isAllSelected = proxies.length > 0 && selectedIds.size === proxies.length;
    const isSomeSelected = selectedIds.size > 0 && selectedIds.size < proxies.length;

    // Helper to get bound accounts for a proxy
    const getBoundAccounts = (proxyId: string) => {
        const boundAccountIds = Object.entries(accountBindings)
            .filter(([_, boundProxyId]) => boundProxyId === proxyId)
            .map(([accountId]) => accountId);

        return boundAccountIds.map(id => accounts.find(a => a.id === id)).filter(Boolean) as Account[];
    };

    return (
        <div className="overflow-hidden">
            <table className="min-w-full divide-y divide-gray-100 dark:divide-gray-800">
                <thead className="bg-gray-50/50 dark:bg-gray-900/50">
                    <tr>
                        <th scope="col" className="px-4 py-3 text-left w-10 pl-6">
                            <input
                                type="checkbox"
                                checked={isAllSelected}
                                ref={input => {
                                    if (input) input.indeterminate = isSomeSelected;
                                }}
                                onChange={handleSelectAll}
                                className="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
                            />
                        </th>
                        <th scope="col" className="px-4 py-3 text-left text-[10px] font-bold text-gray-400 uppercase tracking-widest min-w-[60px]">
                            {t('settings.proxy_pool.column_priority', 'PRI')}
                        </th>
                        <th scope="col" className="px-4 py-3 text-left text-[10px] font-bold text-gray-400 uppercase tracking-widest">
                            {t('settings.proxy_pool.column_status', 'Status')}
                        </th>
                        <th scope="col" className="px-4 py-3 text-left text-[10px] font-bold text-gray-400 uppercase tracking-widest w-1/3">
                            {t('settings.proxy_pool.column_details', 'Proxy Details')}
                        </th>
                        <th scope="col" className="px-4 py-3 text-left text-[10px] font-bold text-gray-400 uppercase tracking-widest">
                            {t('settings.proxy_pool.column_bindings', 'Bindings')}
                        </th>
                        <th scope="col" className="px-4 py-3 text-right text-[10px] font-bold text-gray-400 uppercase tracking-widest pr-6">
                            {t('common.actions', 'Actions')}
                        </th>
                    </tr>
                </thead>
                <tbody className="bg-white dark:bg-gray-900 divide-y divide-gray-50 dark:divide-gray-800">
                    {sortedProxies.length === 0 ? (
                        <td colSpan={6} className="px-6 py-12 text-center text-sm text-gray-500 dark:text-gray-400">
                            <span className="opacity-50">{t('settings.proxy_pool.empty', 'No proxies available.')}</span>
                        </td>
                    ) : (
                        sortedProxies.map((proxy) => {
                            const boundAccounts = getBoundAccounts(proxy.id);

                            return (
                                <tr
                                    key={proxy.id}
                                    className={`group hover:bg-gray-50 dark:hover:bg-gray-800/20 transition-colors ${!proxy.enabled ? 'bg-gray-50/30 dark:bg-gray-900/50' : ''} ${selectedIds.has(proxy.id) ? 'bg-blue-50/50 dark:bg-blue-900/10' : ''}`}
                                >
                                    <td className="px-4 py-3 whitespace-nowrap pl-6">
                                        <input
                                            type="checkbox"
                                            checked={selectedIds.has(proxy.id)}
                                            onChange={() => handleSelectOne(proxy.id)}
                                            className="w-4 h-4 text-blue-600 bg-gray-100 border-gray-300 rounded focus:ring-blue-500 dark:focus:ring-blue-600 dark:ring-offset-gray-800 focus:ring-2 dark:bg-gray-700 dark:border-gray-600"
                                        />
                                    </td>
                                    <td className="px-4 py-3 whitespace-nowrap">
                                        <div className={`flex items-center justify-center w-6 h-6 rounded-full border text-[10px] font-black transition-all shadow-inner ${proxy.enabled
                                            ? 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-700 dark:text-gray-300'
                                            : 'bg-gray-50 dark:bg-gray-900 border-gray-100 dark:border-gray-800 text-gray-400 dark:text-gray-600 opacity-50'
                                            }`}>
                                            {proxy.priority}
                                        </div>
                                    </td>
                                    <td className="px-4 py-3 whitespace-nowrap">
                                        <div className="flex items-center gap-2">
                                            <div className="relative group">
                                                <button
                                                    onClick={() => !isTesting && handleToggleEnabled(proxy.id)}
                                                    className={`relative p-1 rounded-lg transition-all ${!proxy.enabled ? 'opacity-40 hover:opacity-100' : ''}`}
                                                >
                                                    <div className={`w-2 h-2 rounded-full transition-all duration-500 shadow-lg ${!proxy.enabled
                                                        ? 'bg-gray-400'
                                                        : proxy.latency !== undefined && proxy.latency !== null
                                                            ? 'bg-emerald-500 shadow-emerald-500/50'
                                                            : proxy.is_healthy
                                                                ? 'bg-emerald-500 shadow-emerald-500/50'
                                                                : isTesting && !proxy.latency
                                                                    ? 'bg-blue-400 animate-pulse'
                                                                    : 'bg-rose-500 shadow-rose-500/50'
                                                        }`}></div>
                                                </button>
                                            </div>

                                            {/* Status Pill Tag */}
                                            <div className={`px-2 py-0.5 rounded-md text-[10px] font-black uppercase tracking-tighter border transition-all duration-300 ${!proxy.enabled
                                                ? 'bg-gray-100 text-gray-400 border-gray-200 dark:bg-gray-800 dark:border-gray-700'
                                                : proxy.latency !== undefined && proxy.latency !== null
                                                    ? 'bg-emerald-50 text-emerald-600 border-emerald-100 dark:bg-emerald-900/20 dark:text-emerald-400 dark:border-emerald-800/50 shadow-sm'
                                                    : isTesting
                                                        ? 'bg-blue-50 text-blue-600 border-blue-100 dark:bg-blue-900/20 dark:text-blue-400 dark:border-blue-800/50 animate-pulse'
                                                        : proxy.is_healthy
                                                            ? 'bg-emerald-50 text-emerald-600 border-emerald-100 dark:bg-emerald-900/20 dark:text-emerald-400 dark:border-emerald-800/50'
                                                            : 'bg-rose-50 text-rose-600 border-rose-100 dark:bg-rose-900/20 dark:text-rose-400 dark:border-rose-800/50 shadow-sm'
                                                }`}>
                                                {!proxy.enabled
                                                    ? t('settings.proxy_pool.status.inactive', 'Inactive')
                                                    : proxy.latency !== undefined && proxy.latency !== null
                                                        ? `${proxy.latency}ms`
                                                        : isTesting
                                                            ? t('settings.proxy_pool.status.checking', 'Checking')
                                                            : proxy.is_healthy
                                                                ? t('settings.proxy_pool.status.healthy', 'Healthy')
                                                                : t('settings.proxy_pool.status.timeout', 'Timeout')
                                                }
                                            </div>
                                        </div>
                                    </td>
                                    <td className="px-4 py-3">
                                        <div className={`flex flex-col ${!proxy.enabled ? 'opacity-50 grayscale' : ''}`}>
                                            <div className="flex items-center gap-2">
                                                <span className="font-semibold text-sm text-gray-900 dark:text-gray-100">
                                                    {proxy.name}
                                                </span>
                                                {proxy.tags.map(tag => (
                                                    <span key={tag} className="px-1.5 py-0.5 rounded text-[10px] bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400 font-medium tracking-wide">
                                                        {tag}
                                                    </span>
                                                ))}
                                            </div>
                                            <div className="flex items-center gap-1.5 text-xs text-gray-400 dark:text-gray-500 font-mono mt-0.5 max-w-[240px] truncate" title={proxy.url}>
                                                <Globe size={10} />
                                                {proxy.url}
                                            </div>
                                        </div>
                                    </td>
                                    <td className="px-4 py-3 whitespace-nowrap">
                                        {boundAccounts.length > 0 ? (
                                            <div className="flex flex-wrap gap-1 max-w-[140px]" title={`Bound to:\n${boundAccounts.map(a => a.email).join('\n')}`}>
                                                {boundAccounts.slice(0, 2).map(acc => (
                                                    <div key={acc.id} className="px-1.5 py-0.5 rounded-full bg-indigo-50 dark:bg-indigo-900/30 border border-indigo-100 dark:border-indigo-800/50 flex items-center gap-1">
                                                        <div className="w-1.5 h-1.5 rounded-full bg-indigo-500 dark:bg-indigo-400"></div>
                                                        <span className="text-[10px] font-medium text-indigo-700 dark:text-indigo-300">
                                                            {acc.email.split('@')[0].substring(0, 4)}
                                                        </span>
                                                    </div>
                                                ))}
                                                {boundAccounts.length > 2 && (
                                                    <div className="px-1.5 py-0.5 rounded-full bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 flex items-center justify-center text-[10px] font-medium text-gray-500 dark:text-gray-400">
                                                        +{boundAccounts.length - 2}
                                                    </div>
                                                )}
                                            </div>
                                        ) : (
                                            <span className="text-xs text-gray-300 dark:text-gray-700 italic pl-1">{t('common.none', 'None')}</span>
                                        )}
                                    </td>
                                    <td className="px-4 py-3 whitespace-nowrap text-right pr-6">
                                        <div className="flex items-center justify-end gap-1">
                                            <button
                                                onClick={() => handleToggleEnabled(proxy.id)}
                                                className={`p-1.5 transition-colors ${proxy.enabled
                                                    ? 'text-gray-400 hover:text-gray-700 dark:hover:text-gray-200'
                                                    : 'text-gray-300 hover:text-gray-500 dark:text-gray-600 dark:hover:text-gray-400'
                                                    }`}
                                                title={proxy.enabled ? t('common.disable', 'Disable') : t('common.enable', 'Enable')}
                                            >
                                                <Power size={14} />
                                            </button>
                                            <button
                                                onClick={() => handleEdit(proxy)}
                                                className="p-1.5 text-gray-400 hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
                                                title={t('common.edit', 'Edit')}
                                            >
                                                <Edit2 size={14} />
                                            </button>
                                            <button
                                                onClick={() => handleDelete(proxy.id)}
                                                className="p-1.5 text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors"
                                                title={t('common.delete', 'Delete')}
                                            >
                                                <Trash2 size={14} />
                                            </button>
                                        </div>
                                    </td>
                                </tr>
                            )
                        })
                    )}
                </tbody>
            </table>

            {isEditModalOpen && editingProxy && (
                <ProxyEditModal
                    isOpen={isEditModalOpen}
                    onClose={() => setIsEditModalOpen(false)}
                    onSave={handleSaveProxy}
                    initialData={editingProxy}
                    isEditing={true}
                />
            )}
        </div>
    );
}
