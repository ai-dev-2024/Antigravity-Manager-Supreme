import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { request as invoke } from '../../utils/request';
import { Trash2, AlertCircle, Plus, Search, X } from 'lucide-react';

interface IpBlacklistEntry {
    ip_pattern: string;
    reason?: string;
    added_at: number;
    expires_at?: number;
    added_by?: string;
}

interface Props {
    refreshKey?: number;
}

export const BlacklistManager: React.FC<Props> = ({ refreshKey }) => {
    const { t } = useTranslation();
    const [entries, setEntries] = useState<IpBlacklistEntry[]>([]);
    const [loading, setLoading] = useState(false);
    const [search, setSearch] = useState('');

    // Add Modal State
    const [isAddOpen, setIsAddOpen] = useState(false);
    const [newIp, setNewIp] = useState('');
    const [newReason, setNewReason] = useState('');
    const [newExpires, setNewExpires] = useState('');

    const loadBlacklist = async () => {
        setLoading(true);
        try {
            const data = await invoke<IpBlacklistEntry[]>('get_ip_blacklist');
            setEntries(data);
        } catch (e) {
            console.error('Failed to load blacklist', e);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadBlacklist();
    }, [refreshKey]);

    const handleAdd = async () => {
        try {
            let expiresAt = undefined;
            if (newExpires) {
                // Parse expires (e.g. "24h", "7d", or timestamp)
                // generic simple parser for hours
                const hours = parseInt(newExpires);
                if (!isNaN(hours)) {
                    expiresAt = Math.floor(Date.now() / 1000) + hours * 3600;
                }
            }

            await invoke('add_ip_to_blacklist', {
                request: {
                    ipPattern: newIp,
                    reason: newReason || null,
                    expiresAt: expiresAt
                }
            });
            setIsAddOpen(false);
            setNewIp('');
            setNewReason('');
            setNewExpires('');
            loadBlacklist();
        } catch (e) {
            console.error('Failed to add to blacklist', e);
            const errorMsg = String(e);
            if (errorMsg.includes('UNIQUE constraint')) {
                alert(t('security.blacklist.error_duplicate') || 'This IP is already in the blacklist');
            } else if (errorMsg.includes('Invalid IP pattern')) {
                alert(t('security.blacklist.error_invalid_ip') || 'Invalid IP format. Please use IP address or CIDR notation (e.g., 192.168.1.0/24)');
            } else {
                alert(t('security.blacklist.error_add_failed') || 'Failed to add IP: ' + e);
            }
        }
    };

    const handleRemove = async (ipPattern: string) => {
        // 乐观更新：立即从UI中移除
        setEntries(prev => prev.filter(e => e.ip_pattern !== ipPattern));

        try {
            await invoke('remove_ip_from_blacklist', { ipPattern: ipPattern });
        } catch (e) {
            console.error('Failed to remove from blacklist', e);
            // 如果删除失败，重新加载数据恢复UI
            loadBlacklist();
        }
    };

    const filteredEntries = entries.filter(e =>
        e.ip_pattern.includes(search) || (e.reason && e.reason.toLowerCase().includes(search.toLowerCase()))
    );

    return (
        <div className="flex flex-col h-full bg-white dark:bg-base-100 rounded-xl">
            <div className="p-5 border-b border-gray-100 dark:border-base-200 flex items-center gap-4">
                <button
                    onClick={() => setIsAddOpen(true)}
                    className="px-4 py-2 bg-white dark:bg-base-100 text-gray-700 dark:text-gray-300 text-sm font-medium rounded-lg hover:bg-gray-50 dark:hover:bg-base-200 transition-colors flex items-center gap-2 shadow-sm border border-gray-200/50 dark:border-base-300"
                >
                    <Plus size={16} /> {t('security.blacklist.add_ip')}
                </button>

                <div className="relative flex-1 max-w-md">
                    <Search className="absolute left-3 top-2.5 text-gray-400" size={16} />
                    <input
                        type="text"
                        placeholder={t('security.blacklist.search_placeholder')}
                        className="input input-sm input-bordered w-full pl-9"
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                    />
                </div>

                <div className="flex-1"></div>
            </div>

            <div className="flex-1 overflow-auto p-4">
                <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                    {filteredEntries.map(entry => (
                        <div key={entry.ip_pattern} className="bg-white dark:bg-base-100 border border-gray-100 dark:border-base-200 rounded-lg p-4 shadow-sm hover:shadow-md transition-shadow relative group">
                            <div className="flex items-start justify-between mb-2">
                                <h3 className="font-mono font-bold text-lg">{entry.ip_pattern}</h3>
                                <button
                                    onClick={() => handleRemove(entry.ip_pattern)}
                                    className="btn btn-ghost btn-xs text-red-500 opacity-0 group-hover:opacity-100 transition-opacity"
                                >
                                    <Trash2 size={14} />
                                </button>
                            </div>

                            {entry.reason && (
                                <p className="text-sm text-gray-600 dark:text-gray-400 mb-2 flex items-center gap-1">
                                    <AlertCircle size={12} /> {entry.reason}
                                </p>
                            )}

                            <div className="text-xs text-gray-400 flex flex-col gap-1 mt-3 pt-3 border-t border-gray-50 dark:border-base-200">
                                <span>{t('security.blacklist.added_at')}: {new Date(entry.added_at * 1000).toLocaleString()}</span>
                                {entry.expires_at && (
                                    <span className="text-orange-500">{t('security.blacklist.expires_at')}: {new Date(entry.expires_at * 1000).toLocaleString()}</span>
                                )}
                            </div>
                        </div>
                    ))}
                    {!loading && filteredEntries.length === 0 && (
                        <div className="col-span-full text-center py-10 text-gray-400">
                            {t('security.blacklist.no_data')}
                        </div>
                    )}
                </div>
            </div>

            {/* Add Modal */}
            {isAddOpen && (
                <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
                    <div className="bg-white dark:bg-base-100 rounded-lg shadow-xl w-full max-w-md p-6">
                        <div className="flex justify-between items-center mb-4">
                            <h3 className="text-lg font-bold">{t('security.blacklist.add_title')}</h3>
                            <button onClick={() => setIsAddOpen(false)} className="btn btn-ghost btn-sm btn-circle">
                                <X size={18} />
                            </button>
                        </div>

                        <div className="space-y-4">
                            <div>
                                <label className="label">{t('security.blacklist.ip_cidr_label')}</label>
                                <input
                                    type="text"
                                    className="input input-bordered w-full"
                                    placeholder={t('security.blacklist.ip_cidr_placeholder')}
                                    value={newIp}
                                    onChange={e => setNewIp(e.target.value)}
                                />
                            </div>
                            <div>
                                <label className="label">{t('security.blacklist.reason_label')}</label>
                                <input
                                    type="text"
                                    className="input input-bordered w-full"
                                    placeholder={t('security.blacklist.reason_placeholder')}
                                    value={newReason}
                                    onChange={e => setNewReason(e.target.value)}
                                />
                            </div>
                            <div>
                                <label className="label">{t('security.blacklist.expires_label')}</label>
                                <input
                                    type="number"
                                    className="input input-bordered w-full"
                                    placeholder={t('security.blacklist.expires_placeholder')}
                                    value={newExpires}
                                    onChange={e => setNewExpires(e.target.value)}
                                />
                            </div>

                            <div className="flex justify-end gap-3 mt-6">
                                <button
                                    className="px-4 py-2 bg-gray-100 dark:bg-base-200 text-gray-700 dark:text-gray-300 text-sm font-medium rounded-lg hover:bg-gray-200 dark:hover:bg-base-300 transition-colors"
                                    onClick={() => setIsAddOpen(false)}
                                >
                                    {t('security.blacklist.cancel')}
                                </button>
                                <button
                                    className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium rounded-lg shadow-lg shadow-blue-500/20 transition-all disabled:opacity-50 disabled:cursor-not-allowed"
                                    onClick={handleAdd}
                                    disabled={!newIp}
                                >
                                    {t('security.blacklist.add_btn')}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
};
