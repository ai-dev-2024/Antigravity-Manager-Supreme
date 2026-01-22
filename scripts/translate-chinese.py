#!/usr/bin/env python3
"""
Translate Chinese comments and strings in Rust files to English.
Used by GitHub Actions sync workflow.

Supports:
- Line comments (// 中文)
- Block comments (/* 中文 */)
- Doc comments (/// 中文)
- String literals ("中文")

Has offline fallback dictionary for common patterns when API fails.
"""

import os
import re
import sys
from pathlib import Path

# Try to import deep-translator, fall back to offline dictionary
try:
    from deep_translator import GoogleTranslator
    TRANSLATOR_AVAILABLE = True
    translator = GoogleTranslator(source='zh-CN', target='en')
except ImportError:
    TRANSLATOR_AVAILABLE = False
    translator = None
    print("Warning: deep-translator not installed, using offline dictionary only")

# Comprehensive offline translation dictionary
OFFLINE_TRANSLATIONS = {
    # Common verbs
    "获取": "Get",
    "设置": "Set",
    "检查": "Check",
    "处理": "Handle",
    "创建": "Create",
    "删除": "Delete",
    "更新": "Update",
    "返回": "Return",
    "错误": "Error",
    "配置": "Config",
    "初始化": "Initialize",
    "加载": "Load",
    "保存": "Save",
    "发送": "Send",
    "接收": "Receive",
    "解析": "Parse",
    "转换": "Convert",
    "验证": "Validate",
    "执行": "Execute",
    "运行": "Run",
    "停止": "Stop",
    "启动": "Start",
    "关闭": "Close",
    "打开": "Open",
    "读取": "Read",
    "写入": "Write",
    "添加": "Add",
    "移除": "Remove",
    "清除": "Clear",
    "重置": "Reset",
    "刷新": "Refresh",
    "请求": "Request",
    "响应": "Response",
    "连接": "Connect",
    "断开": "Disconnect",
    "监听": "Listen",
    "等待": "Wait",
    "重试": "Retry",
    "失败": "Failed",
    "成功": "Success",
    "完成": "Complete",
    "开始": "Begin",
    "结束": "End",
    "取消": "Cancel",
    "确认": "Confirm",
    "查询": "Query",
    "搜索": "Search",
    "过滤": "Filter",
    "排序": "Sort",
    "合并": "Merge",
    "分割": "Split",
    "复制": "Copy",
    "粘贴": "Paste",
    "剪切": "Cut",
    "撤销": "Undo",
    "重做": "Redo",
    "导入": "Import",
    "导出": "Export",
    "上传": "Upload",
    "下载": "Download",
    "同步": "Sync",
    "异步": "Async",
    "缓存": "Cache",
    "清空": "Empty",
    "格式化": "Format",
    "编码": "Encode",
    "解码": "Decode",
    "压缩": "Compress",
    "解压": "Decompress",
    "加密": "Encrypt",
    "解密": "Decrypt",
    "签名": "Sign",
    "验签": "Verify",
    "认证": "Authenticate",
    "授权": "Authorize",
    "登录": "Login",
    "登出": "Logout",
    "注册": "Register",
    "注销": "Unregister",
    
    # Nouns
    "模块": "Module",
    "函数": "Function",
    "方法": "Method",
    "类型": "Type",
    "接口": "Interface",
    "结构": "Struct",
    "枚举": "Enum",
    "常量": "Constant",
    "变量": "Variable",
    "参数": "Parameter",
    "属性": "Property",
    "字段": "Field",
    "值": "Value",
    "键": "Key",
    "索引": "Index",
    "数组": "Array",
    "列表": "List",
    "字典": "Dictionary",
    "映射": "Mapping",
    "集合": "Set",
    "队列": "Queue",
    "栈": "Stack",
    "树": "Tree",
    "节点": "Node",
    "边": "Edge",
    "图": "Graph",
    "路径": "Path",
    "文件": "File",
    "目录": "Directory",
    "文件夹": "Folder",
    "名称": "Name",
    "扩展名": "Extension",
    "后缀": "Suffix",
    "前缀": "Prefix",
    "内容": "Content",
    "数据": "Data",
    "信息": "Info",
    "消息": "Message",
    "日志": "Log",
    "记录": "Record",
    "状态": "Status",
    "模式": "Mode",
    "选项": "Option",
    "设定": "Setting",
    "偏好": "Preference",
    "版本": "Version",
    "时间": "Time",
    "日期": "Date",
    "时间戳": "Timestamp",
    "超时": "Timeout",
    "延迟": "Delay",
    "间隔": "Interval",
    "周期": "Period",
    "频率": "Frequency",
    "次数": "Count",
    "数量": "Amount",
    "大小": "Size",
    "长度": "Length",
    "宽度": "Width",
    "高度": "Height",
    "深度": "Depth",
    "位置": "Position",
    "偏移": "Offset",
    "范围": "Range",
    "限制": "Limit",
    "阈值": "Threshold",
    "百分比": "Percentage",
    "比率": "Ratio",
    "权重": "Weight",
    "优先级": "Priority",
    "级别": "Level",
    "层级": "Hierarchy",
    "结果": "Result",
    "输出": "Output",
    "输入": "Input",
    "源": "Source",
    "目标": "Target",
    "起始": "Start",
    "终止": "End",
    "客户端": "Client",
    "服务端": "Server",
    "服务器": "Server",
    "代理": "Proxy",
    "网关": "Gateway",
    "端口": "Port",
    "地址": "Address",
    "主机": "Host",
    "域名": "Domain",
    "协议": "Protocol",
    "头部": "Header",
    "正文": "Body",
    "负载": "Payload",
    "令牌": "Token",
    "密钥": "Key",
    "密码": "Password",
    "凭证": "Credential",
    "证书": "Certificate",
    "会话": "Session",
    "上下文": "Context",
    "环境": "Environment",
    "实例": "Instance",
    "对象": "Object",
    "引用": "Reference",
    "指针": "Pointer",
    "句柄": "Handle",
    "回调": "Callback",
    "钩子": "Hook",
    "事件": "Event",
    "信号": "Signal",
    "通知": "Notification",
    "警告": "Warning",
    "提示": "Hint",
    "注释": "Comment",
    "文档": "Document",
    "说明": "Description",
    "示例": "Example",
    "样本": "Sample",
    "测试": "Test",
    "调试": "Debug",
    "跟踪": "Trace",
    "性能": "Performance",
    "效率": "Efficiency",
    "优化": "Optimization",
    "内存": "Memory",
    "线程": "Thread",
    "进程": "Process",
    "任务": "Task",
    "作业": "Job",
    "队列": "Queue",
    "池": "Pool",
    "锁": "Lock",
    "互斥": "Mutex",
    "信号量": "Semaphore",
    "条件": "Condition",
    "原子": "Atomic",
    "并发": "Concurrent",
    "并行": "Parallel",
    "序列": "Sequence",
    "流": "Stream",
    "管道": "Pipeline",
    "通道": "Channel",
    "缓冲": "Buffer",
    "帧": "Frame",
    "包": "Packet",
    "块": "Block",
    "分片": "Shard",
    "片段": "Fragment",
    "段落": "Paragraph",
    "行": "Line",
    "列": "Column",
    "单元": "Cell",
    "格式": "Format",
    "模板": "Template",
    "布局": "Layout",
    "样式": "Style",
    "主题": "Theme",
    "颜色": "Color",
    "字体": "Font",
    "图标": "Icon",
    "图片": "Image",
    "图像": "Picture",
    "视频": "Video",
    "音频": "Audio",
    "声音": "Sound",
    "媒体": "Media",
    "资源": "Resource",
    "资产": "Asset",
    "组件": "Component",
    "插件": "Plugin",
    "扩展": "Extension",
    "库": "Library",
    "框架": "Framework",
    "工具": "Tool",
    "实用程序": "Utility",
    "帮助": "Help",
    "支持": "Support",
    "兼容": "Compatible",
    "特性": "Feature",
    "功能": "Function",
    "能力": "Capability",
    "权限": "Permission",
    "角色": "Role",
    "用户": "User",
    "账户": "Account",
    "账号": "Account",
    "配额": "Quota",
    "限额": "Limit",
    "余额": "Balance",
    "已使用": "Used",
    "剩余": "Remaining",
    "可用": "Available",
    "已废弃": "Deprecated",
    "已过时": "Obsolete",
    "实验性": "Experimental",
    "稳定": "Stable",
    "不稳定": "Unstable",
    "内部": "Internal",
    "外部": "External",
    "公开": "Public",
    "私有": "Private",
    "受保护": "Protected",
    "只读": "ReadOnly",
    "可写": "Writable",
    "必需": "Required",
    "可选": "Optional",
    "默认": "Default",
    "自定义": "Custom",
    "原始": "Raw",
    "派生": "Derived",
    "抽象": "Abstract",
    "具体": "Concrete",
    "虚拟": "Virtual",
    "静态": "Static",
    "动态": "Dynamic",
    "全局": "Global",
    "局部": "Local",
    "临时": "Temporary",
    "持久": "Persistent",
    "瞬态": "Transient",
    "有效": "Valid",
    "无效": "Invalid",
    "空": "Empty",
    "非空": "NonEmpty",
    "真": "True",
    "假": "False",
    "是": "Yes",
    "否": "No",
    "启用": "Enable",
    "禁用": "Disable",
    "激活": "Activate",
    "停用": "Deactivate",
    "显示": "Show",
    "隐藏": "Hide",
    "可见": "Visible",
    "不可见": "Invisible",
    "展开": "Expand",
    "折叠": "Collapse",
    "最大化": "Maximize",
    "最小化": "Minimize",
    "全屏": "Fullscreen",
    "窗口": "Window",
    "对话框": "Dialog",
    "弹出": "Popup",
    "菜单": "Menu",
    "工具栏": "Toolbar",
    "状态栏": "StatusBar",
    "导航": "Navigation",
    "侧边栏": "Sidebar",
    "面板": "Panel",
    "标签": "Tab",
    "页面": "Page",
    "视图": "View",
    "控制器": "Controller",
    "模型": "Model",
    "服务": "Service",
    "仓库": "Repository",
    "工厂": "Factory",
    "构建器": "Builder",
    "适配器": "Adapter",
    "装饰器": "Decorator",
    "代理模式": "Proxy Pattern",
    "单例": "Singleton",
    "观察者": "Observer",
    "订阅者": "Subscriber",
    "发布者": "Publisher",
    "中间件": "Middleware",
    "拦截器": "Interceptor",
    "过滤器": "Filter",
    "处理器": "Handler",
    "解析器": "Parser",
    "序列化器": "Serializer",
    "反序列化器": "Deserializer",
    "转换器": "Converter",
    "映射器": "Mapper",
    "验证器": "Validator",
    "格式化器": "Formatter",
    "生成器": "Generator",
    "迭代器": "Iterator",
    "比较器": "Comparator",
    "哈希": "Hash",
    "摘要": "Digest",
    "校验": "Checksum",
    "签名验证": "Signature Verification",
    
    # Claude/AI specific
    "思考": "Thinking",
    "思考块": "Thinking Block",
    "推理": "Reasoning",
    "对话": "Conversation",
    "聊天": "Chat",
    "助手": "Assistant",
    "系统": "System",
    "模型映射": "Model Mapping",
    "家族映射": "Family Mapping",
    "直通": "Passthrough",
    "透传": "Passthrough",
    "转发": "Forward",
    "路由": "Route",
    "分发": "Dispatch",
    "负载均衡": "Load Balance",
    "轮询": "Round Robin",
    "随机": "Random",
    "最少连接": "Least Connections",
    "健康检查": "Health Check",
    "心跳": "Heartbeat",
    "重连": "Reconnect",
    "降级": "Fallback",
    "熔断": "Circuit Breaker",
    "限流": "Rate Limit",
    "节流": "Throttle",
    "背压": "Backpressure",
    "缓冲区": "Buffer",
    "队列满": "Queue Full",
    "队列空": "Queue Empty",
    "超时重试": "Timeout Retry",
    "最大重试": "Max Retry",
    "指数退避": "Exponential Backoff",
    
    # Error messages
    "无法获取": "Unable to get",
    "无法设置": "Unable to set",
    "无法创建": "Unable to create",
    "无法删除": "Unable to delete",
    "无法连接": "Unable to connect",
    "无法读取": "Unable to read",
    "无法写入": "Unable to write",
    "无法解析": "Unable to parse",
    "无法转换": "Unable to convert",
    "找不到": "Not found",
    "不存在": "Does not exist",
    "已存在": "Already exists",
    "不支持": "Not supported",
    "不允许": "Not allowed",
    "权限不足": "Insufficient permissions",
    "配额耗尽": "Quota exhausted",
    "请求过多": "Too many requests",
    "服务不可用": "Service unavailable",
    "内部错误": "Internal error",
    "网络错误": "Network error",
    "超时错误": "Timeout error",
    "格式错误": "Format error",
    "参数错误": "Parameter error",
    "类型错误": "Type error",
    "验证失败": "Validation failed",
    "认证失败": "Authentication failed",
    "授权失败": "Authorization failed",
    
    # Common phrases
    "使用": "Using",
    "正在": "Currently",
    "尝试": "Trying",
    "已经": "Already",
    "可以": "Can",
    "需要": "Need",
    "必须": "Must",
    "应该": "Should",
    "可能": "May",
    "这是": "This is",
    "这里": "Here",
    "那里": "There",
    "如果": "If",
    "否则": "Else",
    "当": "When",
    "然后": "Then",
    "同时": "Meanwhile",
    "首先": "First",
    "其次": "Second",
    "最后": "Finally",
    "因为": "Because",
    "所以": "So",
    "但是": "But",
    "而且": "And",
    "或者": "Or",
    "除非": "Unless",
    "只有": "Only",
    "所有": "All",
    "一些": "Some",
    "没有": "None",
    "任何": "Any",
    "每个": "Each",
    "另一个": "Another",
    "相同": "Same",
    "不同": "Different",
    "类似": "Similar",
    "更多": "More",
    "更少": "Less",
    "最大": "Maximum",
    "最小": "Minimum",
    "平均": "Average",
    "总计": "Total",
    "当前": "Current",
    "之前": "Before",
    "之后": "After",
    "上面": "Above",
    "下面": "Below",
    "左边": "Left",
    "右边": "Right",
    "中间": "Middle",
    "内部": "Inside",
    "外部": "Outside",
    "新的": "New",
    "旧的": "Old",
    "原有": "Original",
    "修改": "Modified",
    "简单": "Simple",
    "复杂": "Complex",
    "基本": "Basic",
    "高级": "Advanced",
    "通用": "General",
    "专用": "Specialized",
    "核心": "Core",
    "辅助": "Auxiliary",
    "主要": "Main",
    "次要": "Secondary",
    "临界": "Critical",
    "重要": "Important",
    "紧急": "Urgent",
    "普通": "Normal",
    "特殊": "Special",
    "唯一": "Unique",
    "重复": "Duplicate",
    "有效期": "Validity period",
    "过期": "Expired",
    "待处理": "Pending",
    "进行中": "In progress",
    "已完成": "Completed",
    "已取消": "Cancelled",
    "已暂停": "Paused",
    "已恢复": "Resumed",
    
    # Specific to this project
    "音频转录请求": "Audio transcription request",
    "文件名": "File name",
    "文件扩展名": "File extension",
    "方式处理": "Processing method",
    "协议处理器": "Protocol handler",
    "端点处理器": "Endpoint handler",
    "核心端点处理器模块": "Core endpoint handler module",
    "音频转录处理器": "Audio transcription handler",
    "有意义": "Meaningful",
    "后台任务检测": "Background task detection",
    "时间窗口锁定": "Time window locking",
    "已废弃基于内容的哈希": "Deprecated content-based hash",
    "流式响应": "Streaming response",
    "非流式响应": "Non-streaming response",
    "统一处理所有可重试错误": "Unified handling of all retryable errors",
    "不再特殊处理": "No longer special handling",
    "允许账号轮换": "Allow account rotation",
    "递归处理": "Recursive processing",
    "深度递归处理": "Deep recursive processing",
    "校验字段": "Validation field",
    "约束降级为描述中的提示": "Migration logic: Constraints downgraded to hints in description",
    "联合类型": "Union type",
    "移除前提取": "Extract before removal",
    "单字符串且小写": "Single string and lowercase",
    "调用者可以决定": "Caller can decide",
    "设置默认类型": "Set default type",
    "错误处理": "Error handling",
    "生成内容": "Generate content",
    "流式生成内容": "Stream generate content",
    "动态模型列表": "Dynamic model list",
    "内置支持的模型列表关键字": "Built-in supported model list keywords",
    "动态获取所有可用模型列表": "Dynamically get all available model lists",
    "包含内置与用户自定义": "Including built-in and user-defined",
    "自定义精确映射": "Custom exact mapping",
    "优先级最高": "Highest priority",
    "家族分组映射": "Family group mapping",
    "应用家族映射": "Apply family mapping",
    "非请求": "Non-request",
    "原生支持的直通模型": "Natively supported passthrough model",
}

# Chinese character regex
CHINESE_PATTERN = re.compile(r'[\u4e00-\u9fff]+')

def translate_text(text):
    """Translate Chinese text to English using API or offline dictionary."""
    if not text or not CHINESE_PATTERN.search(text):
        return text
    
    # First try offline dictionary for exact matches
    result = text
    for zh, en in sorted(OFFLINE_TRANSLATIONS.items(), key=lambda x: -len(x[0])):
        result = result.replace(zh, en)
    
    # If still contains Chinese, try API
    if CHINESE_PATTERN.search(result) and TRANSLATOR_AVAILABLE:
        try:
            # Extract Chinese segments and translate them
            chinese_matches = CHINESE_PATTERN.findall(result)
            for match in chinese_matches:
                if len(match) >= 2:  # Skip single characters
                    try:
                        translated = translator.translate(match)
                        if translated:
                            result = result.replace(match, translated)
                    except Exception as e:
                        print(f"  API translation failed for '{match}': {e}")
        except Exception as e:
            print(f"  API translation error: {e}")
    
    return result

def process_file(filepath):
    """Process a single Rust file and translate Chinese content."""
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            content = f.read()
    except UnicodeDecodeError:
        try:
            with open(filepath, 'r', encoding='gbk') as f:
                content = f.read()
        except:
            print(f"  Skipping {filepath} - encoding error")
            return False, 0
    
    original_content = content
    translation_count = 0
    
    # Check if file contains Chinese
    if not CHINESE_PATTERN.search(content):
        return False, 0
    
    # Translate line by line to preserve structure
    lines = content.split('\n')
    new_lines = []
    
    for line in lines:
        if CHINESE_PATTERN.search(line):
            translated_line = translate_text(line)
            if translated_line != line:
                translation_count += 1
            new_lines.append(translated_line)
        else:
            new_lines.append(line)
    
    new_content = '\n'.join(new_lines)
    
    if new_content != original_content:
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(new_content)
        return True, translation_count
    
    return False, 0

def main():
    """Main entry point."""
    # Find all .rs files in src-tauri
    src_dir = Path(__file__).parent.parent / 'src-tauri' / 'src'
    
    if not src_dir.exists():
        print(f"Source directory not found: {src_dir}")
        sys.exit(1)
    
    rs_files = list(src_dir.rglob('*.rs'))
    print(f"Found {len(rs_files)} Rust files")
    
    total_modified = 0
    total_translations = 0
    
    for filepath in rs_files:
        relative_path = filepath.relative_to(src_dir.parent.parent)
        modified, count = process_file(filepath)
        if modified:
            print(f"  Translated {count} segments in {relative_path}")
            total_modified += 1
            total_translations += count
    
    print(f"\nSummary: Modified {total_modified} files, {total_translations} translations")
    
    # Check for remaining Chinese
    remaining_chinese = 0
    for filepath in rs_files:
        try:
            with open(filepath, 'r', encoding='utf-8') as f:
                content = f.read()
            if CHINESE_PATTERN.search(content):
                remaining_chinese += len(CHINESE_PATTERN.findall(content))
        except:
            pass
    
    if remaining_chinese > 0:
        print(f"Warning: {remaining_chinese} Chinese segments still remain (may need manual review)")
    else:
        print("Success: All Chinese text translated!")
    
    return 0

if __name__ == '__main__':
    sys.exit(main())
