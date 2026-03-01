import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { request as invoke } from '../../utils/request';
import { Activity, ShieldAlert, Users, Globe } from 'lucide-react';
import { formatCompactNumber } from '../../utils/format';

interface IpRanking {
    client_ip: string;
    request_count: number;
    last_seen: number;
    is_blocked: boolean;
}

interface IpStatsResponse {
    total_requests: number;
    unique_ips: number;
    blocked_requests: number;
    top_ips: IpRanking[];
}

interface IpTokenStats {
    client_ip: string;
    total_tokens: number;
    input_tokens: number;
    output_tokens: number;
    request_count: number;
    username?: string;
}

interface Props {
    refreshKey?: number;
}

export const IpStatistics: React.FC<Props> = ({ refreshKey }) => {
    const { t } = useTranslation();
    const [stats, setStats] = useState<IpStatsResponse | null>(null);
    const [tokenStats, setTokenStats] = useState<IpTokenStats[]>([]);
    const [loading, setLoading] = useState(false);
    const [timeRange, setTimeRange] = useState<number>(24);

    const loadStats = async () => {
        setLoading(true);
        try {
            const [statsData, tokenData] = await Promise.all([
                invoke<IpStatsResponse>('get_ip_stats'),
                invoke<IpTokenStats[]>('get_ip_token_stats', { limit: 20, hours: timeRange })
            ]);
            setStats(statsData);
            setTokenStats(tokenData || []);
        } catch (e) {
            console.error('Failed to load stats', e);
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        loadStats();
    }, [timeRange, refreshKey]);

    const getTimeRangeLabel = () => {
        switch (timeRange) {
            case 1: return t('security.stats.hour');
            case 24: return t('security.stats.day');
            case 168: return t('security.stats.week');
            case 720: return t('security.stats.month');
            default: return `${timeRange} h`;
        }
    };

    if (loading && !stats) {
        return <div className="p-10 text-center"><span className="loading loading-spinner"></span></div>;
    }

    if (!stats) {
        return <div className="p-10 text-center text-gray-500">{t('security.stats.no_data')}</div>;
    }

    const maxReqCount = Math.max(...tokenStats.map(ip => ip.request_count), 1);

    return (
        <div className="h-full flex flex-col overflow-hidden">
            <div className="flex-1 overflow-y-scroll p-6 space-y-6">

                {/* Overview Cards */}
                <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
                    <div className="stat bg-white dark:bg-base-200 shadow rounded-xl border border-gray-100 dark:border-base-300">
                        <div className="stat-figure text-blue-500">
                            <Activity size={32} />
                        </div>
                        <div className="stat-title">{t('security.stats.total_requests')}</div>
                        <div className="stat-value text-blue-500">{formatCompactNumber(stats.total_requests)}</div>
                        <div className="stat-desc">{t('security.stats.total_requests_desc')}</div>
                    </div>

                    <div className="stat bg-white dark:bg-base-200 shadow rounded-xl border border-gray-100 dark:border-base-300">
                        <div className="stat-figure text-purple-500">
                            <Users size={32} />
                        </div>
                        <div className="stat-title">{t('security.stats.unique_ips')}</div>
                        <div className="stat-value text-purple-500">{formatCompactNumber(stats.unique_ips)}</div>
                        <div className="stat-desc">{t('security.stats.unique_ips_desc')}</div>
                    </div>

                    <div className="stat bg-white dark:bg-base-200 shadow rounded-xl border border-gray-100 dark:border-base-300">
                        <div className="stat-figure text-red-500">
                            <ShieldAlert size={32} />
                        </div>
                        <div className="stat-title">{t('security.stats.blocked_requests')}</div>
                        <div className="stat-value text-red-500">{formatCompactNumber(stats.blocked_requests)}</div>
                        <div className="stat-desc">{t('security.stats.blocked_requests_desc')}</div>
                    </div>
                </div>

                <div className="w-full">
                    {/* Combined IP Stats */}
                    <div className="bg-white dark:bg-base-200 rounded-xl shadow-sm border border-gray-100 dark:border-base-300 overflow-hidden">
                        <div className="p-4 border-b border-gray-100 dark:border-base-300 flex items-center justify-between gap-2">
                            <div className="flex items-center gap-2">
                                <Globe size={20} className="text-blue-500" />
                                <h3 className="font-bold text-lg">{t('security.stats.ip_activity_token_usage')} ({getTimeRangeLabel()})</h3>
                            </div>
                            <div className="flex gap-1">
                                <button
                                    className={`btn btn-xs min-w-[48px] ${timeRange === 1 ? 'btn-active btn-primary' : ''}`}
                                    onClick={() => setTimeRange(1)}
                                >{t('security.stats.hour')}</button>
                                <button
                                    className={`btn btn-xs min-w-[48px] ${timeRange === 24 ? 'btn-active btn-primary' : ''}`}
                                    onClick={() => setTimeRange(24)}
                                >{t('security.stats.day')}</button>
                                <button
                                    className={`btn btn-xs min-w-[48px] ${timeRange === 168 ? 'btn-active btn-primary' : ''}`}
                                    onClick={() => setTimeRange(168)}
                                >{t('security.stats.week')}</button>
                                <button
                                    className={`btn btn-xs min-w-[48px] ${timeRange === 720 ? 'btn-active btn-primary' : ''}`}
                                    onClick={() => setTimeRange(720)}
                                >{t('security.stats.month')}</button>
                            </div>
                        </div>
                        <div className="overflow-x-auto">
                            <table className="table w-full">
                                <thead>
                                    <tr>
                                        <th className="w-12">{t('security.stats.rank')}</th>
                                        <th>{t('security.stats.ip_address')}</th>
                                        <th className="w-24">{t('security.logs.username')}</th>
                                        <th className="w-1/4">{t('security.stats.activity_reqs')}</th>
                                        <th className="text-right">{t('security.stats.total_token')}</th>
                                        <th className="text-right text-xs text-gray-500">{t('security.stats.prompt')}</th>
                                        <th className="text-right text-xs text-gray-500">{t('security.stats.completion')}</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {tokenStats.map((ip, index) => {
                                        // Determine color based on usage magnitude
                                        let colorClass = "text-green-500";
                                        if (ip.total_tokens > 1000000) colorClass = "text-red-500 font-bold";
                                        else if (ip.total_tokens > 100000) colorClass = "text-yellow-500 font-bold";
                                        else if (ip.total_tokens > 10000) colorClass = "text-blue-500";

                                        const percentage = Math.min(100, Math.max(0, (ip.request_count / maxReqCount) * 100)) || 0;

                                        return (
                                            <tr key={ip.client_ip} className="hover:bg-gray-50 dark:hover:bg-base-300">
                                                <td className="font-bold text-gray-400">#{index + 1}</td>
                                                <td className="font-mono font-medium">
                                                    {ip.client_ip}
                                                </td>
                                                <td className="font-medium text-blue-600 dark:text-blue-400">{ip.username || '-'}</td>
                                                <td>
                                                    <div className="flex flex-col gap-1">
                                                        <div className="flex justify-between text-xs text-gray-500">
                                                            <span>{formatCompactNumber(ip.request_count)} reqs</span>
                                                            <span>{Math.round(percentage)}%</span>
                                                        </div>
                                                        <div className="w-full bg-gray-100 dark:bg-base-300 rounded-full h-1.5">
                                                            <div
                                                                className="bg-blue-500 h-1.5 rounded-full transition-all duration-500"
                                                                style={{ width: `${percentage}%` }}
                                                            ></div>
                                                        </div>
                                                    </div>
                                                </td>
                                                <td className={`text-right font-mono text-lg ${colorClass}`}>
                                                    {formatCompactNumber(ip.total_tokens)}
                                                </td>
                                                <td className="text-right font-mono text-gray-500 text-xs">
                                                    {formatCompactNumber(ip.input_tokens)}
                                                </td>
                                                <td className="text-right font-mono text-gray-500 text-xs">
                                                    {formatCompactNumber(ip.output_tokens)}
                                                </td>
                                            </tr>
                                        );
                                    })}
                                    {tokenStats.length === 0 && (
                                        <tr>
                                            <td colSpan={7} className="text-center py-8 text-gray-500">
                                                {t('security.stats.no_data')}
                                            </td>
                                        </tr>
                                    )}
                                </tbody>
                            </table>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    );
};
