import React, { useState } from 'react';
import { useNetworkMonitorStore, NetworkRequest } from '../../stores/networkMonitorStore';
import { X, Play, Pause, Trash2, Activity, ChevronDown } from 'lucide-react';
import { useTranslation } from 'react-i18next';

const NetworkMonitor: React.FC = () => {
    const { requests, isOpen, setIsOpen, isRecording, toggleRecording, clearRequests } = useNetworkMonitorStore();
    const [selectedRequest, setSelectedRequest] = useState<NetworkRequest | null>(null);
    const { t } = useTranslation();

    // If not open, show a small floating button
    if (!isOpen) {
        return (
            <div className="fixed bottom-4 right-4 z-50">
                <button
                    onClick={() => setIsOpen(true)}
                    className="btn btn-circle btn-primary shadow-lg"
                    title={t('monitor.network.open', 'ネットワークモニターを開く')}
                >
                    <Activity size={24} />
                    {requests.filter(r => r.status === 'pending').length > 0 && (
                        <span className="absolute -top-1 -right-1 flex h-3 w-3">
                            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-secondary opacity-75"></span>
                            <span className="relative inline-flex rounded-full h-3 w-3 bg-secondary"></span>
                        </span>
                    )}
                </button>
            </div>
        );
    }

    return (
        <div className="fixed inset-0 z-50 flex flex-col bg-base-100/95 backdrop-blur shadow-2xl transition-transform duration-300 pointer-events-auto border-t border-base-300 md:w-2/3 md:inset-y-0 md:right-0 md:left-auto md:border-t-0 md:border-l">
            {/* Header */}
            <div className="flex items-center justify-between p-4 border-b border-base-300 bg-base-200/50">
                <div className="flex items-center gap-2">
                    <Activity className="text-primary" size={20} />
                    <h2 className="font-bold text-lg">{t('monitor.network.title', 'ネットワークモニター')}</h2>
                    <span className="badge badge-sm">
                        {t('monitor.network.requests_count', '{{count}} 件', { count: requests.length })}
                    </span>
                </div>
                <div className="flex items-center gap-2">
                    <button
                        onClick={toggleRecording}
                        className={`btn btn-sm btn-circle ${isRecording ? 'btn-error' : 'btn-success'}`}
                        title={isRecording
                            ? t('monitor.network.stop_recording', '記録を停止')
                            : t('monitor.network.start_recording', '記録を開始')}
                    >
                        {isRecording ? <Pause size={14} /> : <Play size={14} />}
                    </button>
                    <button
                        onClick={clearRequests}
                        className="btn btn-sm btn-circle btn-ghost"
                        title={t('monitor.network.clear_requests', 'リクエストをクリア')}
                    >
                        <Trash2 size={16} />
                    </button>
                    <button
                        onClick={() => setIsOpen(false)}
                        className="btn btn-sm btn-circle btn-ghost"
                    >
                        <X size={20} />
                    </button>
                </div>
            </div>

            {/* Main Content */}
            <div className="flex-1 flex overflow-hidden">
                {/* Request List */}
                <div className={`flex-1 overflow-y-auto border-r border-base-300 ${selectedRequest ? 'hidden md:block md:w-1/2' : 'w-full'}`}>
                    <table className="table table-xs table-pin-rows w-full">
                        <thead>
                            <tr className="bg-base-200">
                                <th className="w-16">{t('monitor.network.table.status', '状態')}</th>
                                <th>{t('monitor.network.table.command', 'コマンド')}</th>
                                <th className="w-20 text-right">{t('monitor.network.table.time', '時刻')}</th>
                                <th className="w-20 text-right">{t('monitor.network.table.duration', '所要時間')}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {requests.map((req) => (
                                <tr
                                    key={req.id}
                                    className={`cursor-pointer hover:bg-base-200 ${selectedRequest?.id === req.id ? 'bg-primary/10' : ''}`}
                                    onClick={() => setSelectedRequest(req)}
                                >
                                    <td>
                                        <BadgeStatus status={req.status} />
                                    </td>
                                    <td className="font-mono text-xs truncate max-w-[200px]" title={req.cmd}>
                                        {req.cmd}
                                    </td>
                                    <td className="text-right text-xs opacity-70">
                                        {new Date(req.startTime).toLocaleTimeString()}
                                    </td>
                                    <td className="text-right text-xs opacity-70">
                                        {req.duration ? `${req.duration}ms` : '-'}
                                    </td>
                                </tr>
                            ))}
                            {requests.length === 0 && (
                                <tr>
                                    <td colSpan={4} className="text-center py-8 opacity-50">
                                        {t('monitor.network.empty', '記録されたリクエストはありません')}
                                    </td>
                                </tr>
                            )}
                        </tbody>
                    </table>
                </div>

                {/* Details Panel */}
                {selectedRequest && (
                    <div className="flex-1 md:w-1/2 overflow-y-auto bg-base-100 flex flex-col absolute inset-0 md:static z-10 w-full">
                        <div className="flex items-center justify-between p-2 border-b border-base-300 bg-base-200/30 md:hidden">
                            <button onClick={() => setSelectedRequest(null)} className="btn btn-sm btn-ghost">
                                <ChevronDown size={16} className="rotate-90" /> {t('common.back', '戻る')}
                            </button>
                            <span className="font-mono text-xs">{selectedRequest.cmd}</span>
                        </div>

                        <div className="p-4 space-y-4">
                            <div>
                                <h3 className="text-xs font-bold uppercase opacity-50 mb-1">{t('monitor.network.sections.general', '概要')}</h3>
                                <div className="bg-base-200 rounded p-2 text-xs space-y-1">
                                    <div className="flex justify-between">
                                        <span className="opacity-70">ID:</span>
                                        <span className="font-mono select-all">{selectedRequest.id}</span>
                                    </div>
                                    <div className="flex justify-between">
                                        <span className="opacity-70">{t('monitor.network.fields.status', '状態')}:</span>
                                        <BadgeStatus status={selectedRequest.status} />
                                    </div>
                                    <div className="flex justify-between">
                                        <span className="opacity-70">{t('monitor.network.fields.start_time', '開始時刻')}:</span>
                                        <span>{new Date(selectedRequest.startTime).toLocaleString()}</span>
                                    </div>
                                    {selectedRequest.duration && (
                                        <div className="flex justify-between">
                                            <span className="opacity-70">{t('monitor.network.fields.duration', '所要時間')}:</span>
                                            <span>{selectedRequest.duration}ms</span>
                                        </div>
                                    )}
                                </div>
                            </div>

                            <div>
                                <h3 className="text-xs font-bold uppercase opacity-50 mb-1">{t('monitor.network.sections.request_args', 'リクエスト引数')}</h3>
                                <JsonView data={selectedRequest.args} />
                            </div>

                            <div>
                                <h3 className="text-xs font-bold uppercase opacity-50 mb-1">
                                    {selectedRequest.status === 'error'
                                        ? t('monitor.network.sections.error_details', 'エラー詳細')
                                        : t('monitor.network.sections.response', 'レスポンス')}
                                </h3>
                                {(selectedRequest.response || selectedRequest.error) ? (
                                    <JsonView
                                        data={selectedRequest.status === 'error' ? selectedRequest.error : selectedRequest.response}
                                        isError={selectedRequest.status === 'error'}
                                    />
                                ) : (
                                    <div className="text-xs opacity-50 italic">{t('monitor.network.waiting', '応答待ち...')}</div>
                                )}
                            </div>
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
};

const BadgeStatus = ({ status }: { status: NetworkRequest['status'] }) => {
    const { t } = useTranslation();
    switch (status) {
        case 'success':
            return <span className="badge badge-xs badge-success">200</span>;
        case 'error':
            return <span className="badge badge-xs badge-error">{t('monitor.network.badge_error', 'エラー')}</span>;
        case 'pending':
            return <span className="loading loading-spinner loading-xs text-warning"></span>;
    }
};

const JsonView = ({ data, isError = false }: { data: any, isError?: boolean }) => {
    const { t } = useTranslation();
    if (data === undefined || data === null) {
        return <div className="text-xs opacity-50 italic">{t('common.empty', '空')}</div>;
    }

    return (
        <div className={`mockup-code bg-base-300 text-xs min-h-0 ${isError ? 'border border-error/50' : ''}`}>
            <pre className="px-4 py-2 overflow-x-auto">
                <code>{JSON.stringify(data, null, 2)}</code>
            </pre>
        </div>
    );
};

export default NetworkMonitor;
