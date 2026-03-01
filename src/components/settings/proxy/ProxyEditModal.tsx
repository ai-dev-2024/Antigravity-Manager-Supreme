
import { useState, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { X, Save, Plus } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ProxyEntry } from '../../../types/config';
import { generateUUID } from '../../../utils/uuid';

interface ProxyEditModalProps {
    isOpen: boolean;
    onClose: () => void;
    onSave: (entry: ProxyEntry) => void;
    initialData?: ProxyEntry;
    isEditing: boolean;
}

export default function ProxyEditModal({ isOpen, onClose, onSave, initialData, isEditing }: ProxyEditModalProps) {
    const { t } = useTranslation();
    const [formData, setFormData] = useState<ProxyEntry>({
        id: generateUUID(),
        name: '',
        url: '',
        priority: 0,
        enabled: true,
        tags: [],
        auth: {
            username: '',
            password: ''
        },
        max_accounts: 0,
        is_healthy: true,
        health_check_url: ''
    });

    const [tagInput, setTagInput] = useState('');

    useEffect(() => {
        if (isOpen) {
            if (isEditing && initialData) {
                setFormData(JSON.parse(JSON.stringify(initialData)));
            } else {
                setFormData({
                    id: generateUUID(),
                    name: '',
                    url: '',
                    priority: 0,
                    enabled: true,
                    tags: [],
                    auth: { username: '', password: '' },
                    max_accounts: 0,
                    is_healthy: true,
                    health_check_url: ''
                });
            }
        }
    }, [isOpen, initialData, isEditing]);

    const handleSave = () => {
        if (!formData.name || !formData.url) {
            // Basic validation
            return;
        }
        // Clean up auth if empty
        const entryToSave = { ...formData };
        if (!entryToSave.auth?.username && !entryToSave.auth?.password) {
            entryToSave.auth = undefined;
        }
        onSave(entryToSave);
        onClose();
    };

    const addTag = () => {
        if (tagInput.trim() && !formData.tags.includes(tagInput.trim())) {
            setFormData(prev => ({
                ...prev,
                tags: [...prev.tags, tagInput.trim()]
            }));
            setTagInput('');
        }
    };

    const removeTag = (tag: string) => {
        setFormData(prev => ({
            ...prev,
            tags: prev.tags.filter(t => t !== tag)
        }));
    };

    if (!isOpen) return null;

    return createPortal(
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm">
            <div className="bg-white dark:bg-gray-800 rounded-lg shadow-xl w-full max-w-lg mx-4 flex flex-col max-h-[90vh]">
                <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
                    <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
                        {isEditing ? t('settings.proxy_pool.edit_proxy', 'Edit Proxy') : t('settings.proxy_pool.add_proxy', 'Add Proxy')}
                    </h3>
                    <button onClick={onClose} className="text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200">
                        <X size={20} />
                    </button>
                </div>

                <div className="p-6 space-y-4 overflow-y-auto">
                    {/* Basic Info */}
                    <div className="grid grid-cols-2 gap-4">
                        <div className="col-span-2">
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                {t('settings.proxy_pool.name', 'Name')}
                            </label>
                            <input
                                type="text"
                                value={formData.name}
                                onChange={e => setFormData({ ...formData, name: e.target.value })}
                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
                                placeholder={t('settings.proxy_pool.name', 'Name')}
                            />
                        </div>
                        <div className="col-span-2">
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                {t('settings.proxy_pool.url', 'Proxy URL')}
                            </label>
                            <input
                                type="text"
                                value={formData.url}
                                onChange={e => setFormData({ ...formData, url: e.target.value })}
                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white focus:ring-2 focus:ring-blue-500"
                                placeholder="http://127.0.0.1:7890"
                            />
                        </div>
                    </div>

                    {/* Auth */}
                    <div className="grid grid-cols-2 gap-4 border-t border-gray-200 dark:border-gray-700 pt-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                {t('settings.proxy_pool.username', 'Username')} ({t('common.optional', 'Optional')})
                            </label>
                            <input
                                type="text"
                                value={formData.auth?.username || ''}
                                onChange={e => setFormData({
                                    ...formData,
                                    auth: { ...formData.auth!, username: e.target.value }
                                })}
                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                            />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                {t('settings.proxy_pool.password', 'Password')} ({t('common.optional', 'Optional')})
                            </label>
                            <input
                                type="password"
                                value={formData.auth?.password || ''}
                                onChange={e => setFormData({
                                    ...formData,
                                    auth: { ...formData.auth!, password: e.target.value }
                                })}
                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                            />
                        </div>
                    </div>

                    {/* Advanced */}
                    <div className="grid grid-cols-2 gap-4 border-t border-gray-200 dark:border-gray-700 pt-4">
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                {t('settings.proxy_pool.priority', 'Priority')} ({t('settings.proxy_pool.priority_hint', 'Lower is better')})
                            </label>
                            <input
                                type="number"
                                value={formData.priority}
                                onChange={e => setFormData({ ...formData, priority: parseInt(e.target.value) || 0 })}
                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                            />
                        </div>
                        <div>
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                {t('settings.proxy_pool.max_accounts', 'Max Accounts')} ({t('settings.proxy_pool.max_accounts_hint', '0 = Unlimited')})
                            </label>
                            <input
                                type="number"
                                value={formData.max_accounts || 0}
                                onChange={e => setFormData({ ...formData, max_accounts: parseInt(e.target.value) || 0 })}
                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                            />
                        </div>
                        <div className="col-span-2">
                            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                                {t('settings.proxy_pool.health_check_url', 'Health Check URL')}
                            </label>
                            <input
                                type="text"
                                value={formData.health_check_url || ''}
                                onChange={e => setFormData({ ...formData, health_check_url: e.target.value })}
                                className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                placeholder="https://www.google.com"
                            />
                        </div>
                    </div>

                    {/* Tags */}
                    <div className="border-t border-gray-200 dark:border-gray-700 pt-4">
                        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                            {t('settings.proxy_pool.tags', 'Tags')}
                        </label>
                        <div className="flex gap-2 mb-2">
                            <input
                                type="text"
                                value={tagInput}
                                onChange={e => setTagInput(e.target.value)}
                                onKeyDown={e => e.key === 'Enter' && addTag()}
                                className="flex-1 px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md bg-white dark:bg-gray-700 text-gray-900 dark:text-white"
                                placeholder={t('settings.proxy_pool.add_tag_placeholder', 'Add tag...')}
                            />
                            <button onClick={addTag} className="p-2 bg-blue-600 text-white rounded-md hover:bg-blue-700">
                                <Plus size={20} />
                            </button>
                        </div>
                        <div className="flex flex-wrap gap-2">
                            {formData.tags.map(tag => (
                                <span key={tag} className="px-2 py-1 bg-gray-100 dark:bg-gray-700 rounded-md text-sm flex items-center gap-1">
                                    {tag}
                                    <button onClick={() => removeTag(tag)} className="text-red-500 hover:text-red-700"><X size={14} /></button>
                                </span>
                            ))}
                        </div>
                    </div>

                </div>

                <div className="p-4 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 rounded-b-lg flex justify-end gap-3">
                    <button
                        onClick={onClose}
                        className="px-4 py-2 text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-lg transition-colors"
                    >
                        {t('common.cancel', 'Cancel')}
                    </button>
                    <button
                        onClick={handleSave}
                        className="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-lg transition-colors flex items-center gap-2"
                    >
                        <Save size={18} />
                        {t('common.save', 'Save')}
                    </button>
                </div>
            </div>
        </div>,
        document.body
    );
}
