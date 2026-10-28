# dioman API 覆盖矩阵

> 手工分析产出，尚无生成脚本；由人工与 axp 台账脚本合并后再统一

分析日期：2026-10-27。仅静态阅读；未运行测试、accept、服务器或 netem。

口径：以 `lib/dioman.dart` 的显式公开导出、自有类型和注释为准；不展开 Dio 继承 API。构造参数与同名属性合并为一个语义 API，JSON 的 `symbols` 保留对应声明；请求 Options、回调类型、枚举值、兼容 API 单列。`core` 是公共装配协议，另计。借用 dioman 现有 `accept/api-inventory.json` 辅助定位，并核对源码；未运行其生成器。

“覆盖率”是**有针对性场景的 API 关联率**，不是运行通过率或全契约覆盖率。默认安装、固定默认值、类型被引用、宿主补偿均不自动算覆盖；存疑计入缺口。任意回调、全部边界及全部取值仍需补齐，不能据此宣布 100%。

依据：[场景约定](AGENTS.md)、[矩阵](MATRIX.md)、[契约](CONTRACT.md)、[schema](scenario.schema.json)、[场景目录](scenarios/README.md)，以及 dioman 的 `lib/src/*.dart`、`accept/mapping.md`、`accept/client.dart`。未读写 axp 的台账脚本或台账文件。

## 每插件汇总

| 插件 | API 数 | 有场景 | 关联率 | 缺口（含存疑） |
|---|---:|---:|---:|---:|
| envs | 22 | 0 | 0.00% | 22 |
| timeout | 26 | 1 | 3.85% | 25（存疑 1） |
| repath | 13 | 1 | 7.69% | 12 |
| filter | 15 | 1 | 6.67% | 14 |
| key | 19 | 5 | 26.32% | 14（存疑 1） |
| normalize | 26 | 2 | 7.69% | 24 |
| cache | 43 | 8 | 18.60% | 35 |
| offline | 29 | 3 | 10.34% | 26（存疑 1） |
| share | 28 | 6 | 21.43% | 22 |
| mock | 22 | 4 | 18.18% | 18 |
| cancel | 17 | 3 | 17.65% | 14 |
| loading | 20 | 7 | 35.00% | 13 |
| auth | 49 | 10 | 20.41% | 39 |
| retry | 42 | 7 | 16.67% | 35（存疑 5） |
| breaker | 43 | 7 | 16.28% | 36 |
| log | 26 | 0 | 0.00% | 26 |
| notify | 12 | 5 | 41.67% | 7 |
| core | 74 | 0 | 0.00% | 74 |

17 插件共 **452 API，70 有场景，382 缺口**；加公共 API 共 **526 API，70 有场景，456 缺口**。存疑 8 项均未计覆盖。

关键限制：

- `timeout.total_ms` 和请求 `timeout_ms` 的总预算来自宿主 Timer/CancelToken，不是 DiomanTimeout 原生总超时 API；`probe` 固定 excellent，未验证其它档位。黑洞已完成 TCP 连接，`timeout.connect-blackhole` 的 timeout 分类不足以证明 connect 字段生效。
- offline 的 `/t/ping` 由宿主执行；插件本身接收在线布尔值和事件流。执行器固定 maxWait=0，现有恢复场景没有证明排队后的事件冲刷。
- retry 全局 max/methods/delay/shouldRetry/respectRetryAfter 同时写入请求 Options，遮蔽构造值；只计请求字段，构造回退标存疑。`statuses` 映射到 shouldRetry，未覆盖原生 statusCodes。数值/false 简写也先转 Options，未覆盖原生 extra 简写。
- notify 的 silent 是宿主 stringify 策略；原生没有 silent extra。业务错误分类含宿主 EnvelopeTransformer，不等于 normalize、auth、retry、breaker 默认就能处理所有信封。
- `pending_after` 来自宿主请求集合，不证明 Loading.activeCount 或 Offline.pending；clear_cache 只证明 Cache.clear，不证明持久化策略。

## 逐 API 明细

“部分”表示现有 schema/映射可表达预置子集；“无法表达”也是缺口。API 的 `opt` 为构造级、`extra` 为请求级，通常位于 `extra[pluginName]` 的 Options 对象；key 的配置键是 `dioman:qid`，结果协议键是 `dioman:key`。envs/cancel 的 Options 仅构造期使用，NotifyOptions 是弃用空类型。

### envs

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `envs.export.DiomanEnvsOptions` | 导出类型：公开类型 DiomanEnvsOptions | 无法表达 | — | 缺口 |
| `envs.ctor.DiomanEnvsOptions.new` | 构造方法：构造 DiomanEnvsOptions.new | 无法表达 | — | 缺口 |
| `envs.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `envs.export.EnvRule` | 导出类型：公开类型 EnvRule | 无法表达 | — | 缺口 |
| `envs.ctor.EnvRule.new` | 构造方法：构造 EnvRule.new | 无法表达 | — | 缺口 |
| `envs.nested.EnvRule.rule` | 嵌套字段：环境匹配回调 | 无法表达 | — | 缺口 |
| `envs.nested.EnvRule.config` | 嵌套字段：解析后的构造配置 | 无法表达 | — | 缺口 |
| `envs.export.DiomanEnvs` | 导出类型：公开类型 DiomanEnvs | 无法表达 | — | 缺口 |
| `envs.ctor.DiomanEnvs.new` | 构造方法：构造 DiomanEnvs.new | 无法表达 | — | 缺口 |
| `envs.opt.rules` | 构造选项：有序环境规则列表，首条命中生效 | 无法表达 | — | 缺口 |
| `envs.opt.dio` | 构造选项：绑定或解绑宿主 Dio | 无法表达 | — | 缺口 |
| `envs.prop.DiomanEnvs.config` | 公开属性：解析后的构造配置 | 无法表达 | — | 缺口 |
| `envs.prop.DiomanEnvs.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `envs.prop.DiomanEnvs.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `envs.method.DiomanEnvs.onRequest` | 公开方法：请求拦截入口 | 无法表达 | — | 缺口 |
| `envs.method.DiomanEnvs.apply` | 公开方法：求值环境规则并应用至 Dio | 无法表达 | — | 缺口 |
| `envs.nested.EnvRule.config.baseUrl` | 嵌套字段：环境规则 BaseOptions.baseUrl 覆盖 | 无法表达 | — | 缺口 |
| `envs.nested.EnvRule.config.connectTimeout` | 嵌套字段：环境规则 BaseOptions.connectTimeout 覆盖 | 无法表达 | — | 缺口 |
| `envs.nested.EnvRule.config.receiveTimeout` | 嵌套字段：环境规则 BaseOptions.receiveTimeout 覆盖 | 无法表达 | — | 缺口 |
| `envs.nested.EnvRule.config.sendTimeout` | 嵌套字段：环境规则 BaseOptions.sendTimeout 覆盖 | 无法表达 | — | 缺口 |
| `envs.nested.EnvRule.config.headers` | 嵌套字段：环境规则 BaseOptions.headers 覆盖 | 无法表达 | — | 缺口 |
| `envs.nested.EnvRule.config.responseType` | 嵌套字段：环境规则 BaseOptions.responseType 覆盖 | 无法表达 | — | 缺口 |

### timeout

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `timeout.export.NetworkQuality` | 导出枚举：公开枚举 NetworkQuality | 无法表达 | — | 缺口 |
| `timeout.enum.NetworkQuality.excellent` | 枚举值：枚举取值 NetworkQuality.excellent | 无法表达 | — | 缺口 |
| `timeout.enum.NetworkQuality.good` | 枚举值：枚举取值 NetworkQuality.good | 无法表达 | — | 缺口 |
| `timeout.enum.NetworkQuality.poor` | 枚举值：枚举取值 NetworkQuality.poor | 无法表达 | — | 缺口 |
| `timeout.enum.NetworkQuality.none` | 枚举值：枚举取值 NetworkQuality.none | 无法表达 | — | 缺口 |
| `timeout.export.DiomanTimeouts` | 导出类型：公开类型 DiomanTimeouts | 无法表达 | — | 缺口 |
| `timeout.ctor.DiomanTimeouts.new` | 构造方法：构造 DiomanTimeouts.new | 无法表达 | — | 缺口 |
| `timeout.value.timeouts.connect` | 嵌套字段：连接超时；空值不覆盖；黑洞接受 TCP 连接；timeout 分类及场景总预算不足以独立证明连接超时字段生效。 | 部分 | — | 存疑 |
| `timeout.value.timeouts.receive` | 嵌套字段：接收超时；空值不覆盖 | 部分 | `timeout.receive-slow-body`、`timeout.receive-ok-fast-chunks` | 已关联 |
| `timeout.value.timeouts.send` | 嵌套字段：发送超时；空值不覆盖 | 无法表达 | — | 缺口 |
| `timeout.export.DiomanTimeoutOptions` | 导出类型：公开类型 DiomanTimeoutOptions | 无法表达 | — | 缺口 |
| `timeout.ctor.DiomanTimeoutOptions.new` | 构造方法：构造 DiomanTimeoutOptions.new | 无法表达 | — | 缺口 |
| `timeout.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `timeout.extra.timeouts` | 请求级选项：按网络质量档位配置超时；请求级按档合并 | 无法表达 | — | 缺口 |
| `timeout.export.DiomanTimeout` | 导出类型：公开类型 DiomanTimeout | 无法表达 | — | 缺口 |
| `timeout.ctor.DiomanTimeout.new` | 构造方法：构造 DiomanTimeout.new | 无法表达 | — | 缺口 |
| `timeout.opt.probe` | 构造选项：每请求返回网络质量的回调 | 无法表达 | — | 缺口 |
| `timeout.opt.timeouts` | 构造选项：按网络质量档位配置超时；请求级按档合并 | 无法表达 | — | 缺口 |
| `timeout.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `timeout.prop.DiomanTimeout.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `timeout.prop.DiomanTimeout.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `timeout.method.DiomanTimeout.onRequest` | 公开方法：请求拦截入口 | 无法表达 | — | 缺口 |
| `timeout.value.timeouts.excellent` | 嵌套字段：网络质量 excellent 的档位映射 | 无法表达 | — | 缺口 |
| `timeout.value.timeouts.good` | 嵌套字段：网络质量 good 的档位映射 | 无法表达 | — | 缺口 |
| `timeout.value.timeouts.poor` | 嵌套字段：网络质量 poor 的档位映射 | 无法表达 | — | 缺口 |
| `timeout.value.timeouts.none` | 嵌套字段：网络质量 none 的档位映射 | 无法表达 | — | 缺口 |

### repath

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `repath.export.DiomanRepathOptions` | 导出类型：公开类型 DiomanRepathOptions | 无法表达 | — | 缺口 |
| `repath.ctor.DiomanRepathOptions.new` | 构造方法：构造 DiomanRepathOptions.new | 无法表达 | — | 缺口 |
| `repath.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `repath.extra.removeKey` | 请求级选项：替换路径变量后是否删除源参数 | 无法表达 | — | 缺口 |
| `repath.extra.pattern` | 请求级选项：路径变量匹配正则 | 无法表达 | — | 缺口 |
| `repath.export.DiomanRepath` | 导出类型：公开类型 DiomanRepath | 无法表达 | — | 缺口 |
| `repath.ctor.DiomanRepath.new` | 构造方法：构造 DiomanRepath.new | 无法表达 | — | 缺口 |
| `repath.opt.removeKey` | 构造选项：替换路径变量后是否删除源参数 | 无法表达 | — | 缺口 |
| `repath.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `repath.opt.pattern` | 构造选项：路径变量匹配正则 | 无法表达 | — | 缺口 |
| `repath.prop.DiomanRepath.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `repath.prop.DiomanRepath.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `repath.method.DiomanRepath.onRequest` | 公开方法：请求拦截入口 | 部分 | `repath.basic`、`repath.from-body`、`repath.multi-vars` | 已关联 |

### filter

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `filter.export.DiomanFilterOptions` | 导出类型：公开类型 DiomanFilterOptions | 无法表达 | — | 缺口 |
| `filter.ctor.DiomanFilterOptions.new` | 构造方法：构造 DiomanFilterOptions.new | 无法表达 | — | 缺口 |
| `filter.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `filter.extra.ignoreKeys` | 请求级选项：保留的参数键；请求配置取并集 | 无法表达 | — | 缺口 |
| `filter.extra.ignoreValues` | 请求级选项：保留的参数值；请求配置取并集 | 无法表达 | — | 缺口 |
| `filter.extra.predicate` | 请求级选项：参数过滤回调 | 无法表达 | — | 缺口 |
| `filter.export.DiomanFilter` | 导出类型：公开类型 DiomanFilter | 无法表达 | — | 缺口 |
| `filter.ctor.DiomanFilter.new` | 构造方法：构造 DiomanFilter.new | 无法表达 | — | 缺口 |
| `filter.opt.predicate` | 构造选项：参数过滤回调 | 无法表达 | — | 缺口 |
| `filter.opt.ignoreKeys` | 构造选项：保留的参数键；请求配置取并集 | 无法表达 | — | 缺口 |
| `filter.opt.ignoreValues` | 构造选项：保留的参数值；请求配置取并集 | 无法表达 | — | 缺口 |
| `filter.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `filter.prop.DiomanFilter.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `filter.prop.DiomanFilter.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `filter.method.DiomanFilter.onRequest` | 公开方法：请求拦截入口 | 部分 | `filter.keeps-zero-false`、`key.filter-before-key` | 已关联 |

### key

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `key.export.kKey` | 导出常量：协议常量 kKey | 无法表达 | — | 缺口 |
| `key.export.DiomanKeyOptions` | 导出类型：公开类型 DiomanKeyOptions | 无法表达 | — | 缺口 |
| `key.ctor.DiomanKeyOptions.new` | 构造方法：构造 DiomanKeyOptions.new | 无法表达 | — | 缺口 |
| `key.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 部分 | [G1](#g1) | 已关联 |
| `key.extra.key` | 请求级选项：显式请求键 | 部分 | `cache.custom-key-shared`、`key.explicit-string-merges` | 已关联 |
| `key.extra.fastMode` | 请求级选项：快速键仅含方法、来源与路径；深键含参数与请求体；只验证 deep/false 路径；simple/true 无针对性场景，仍有取值缺口。 | 部分 | `cache.diff-body-no-mixup`、`key.body-diff-not-merged` | 已关联 |
| `key.extra.ignores` | 请求级选项：深键忽略的参数键；请求配置取并集 | 无法表达 | — | 缺口 |
| `key.extra.builder` | 请求级选项：自定义请求键回调 | 无法表达 | — | 缺口 |
| `key.export.DiomanKey` | 导出类型：公开类型 DiomanKey | 无法表达 | — | 缺口 |
| `key.ctor.DiomanKey.new` | 构造方法：构造 DiomanKey.new | 无法表达 | — | 缺口 |
| `key.opt.fastMode` | 构造选项：快速键仅含方法、来源与路径；深键含参数与请求体 | 无法表达 | — | 缺口 |
| `key.opt.ignores` | 构造选项：深键忽略的参数键；请求配置取并集 | 无法表达 | — | 缺口 |
| `key.opt.enabled` | 构造选项：启用开关（空值继承插件配置）；执行器每请求写 enabled，遮蔽构造级开关。 | 部分 | — | 存疑 |
| `key.opt.builder` | 构造选项：自定义请求键回调 | 无法表达 | — | 缺口 |
| `key.opt.namespace` | 构造选项：动态用户作用域回调；无请求级覆盖 | 部分 | `cache.user-scope-no-leak`、`cache.user-scope-same-user-hits` | 已关联 |
| `key.prop.DiomanKey.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `key.prop.DiomanKey.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `key.prop.DiomanKey.ordering` | 公开属性：插件声明的顺序与依赖约束 | 无法表达 | — | 缺口 |
| `key.method.DiomanKey.onRequest` | 公开方法：请求拦截入口 | 部分 | `key.query-order-insensitive`、`key.body-diff-not-merged`、`key.method-diff-not-merged` | 已关联 |

### normalize

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `normalize.export.DiomanNormalizeOptions` | 导出类型：公开类型 DiomanNormalizeOptions | 无法表达 | — | 缺口 |
| `normalize.ctor.DiomanNormalizeOptions.new` | 构造方法：构造 DiomanNormalizeOptions.new | 无法表达 | — | 缺口 |
| `normalize.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 部分 | `normalize.per-request-off` | 已关联 |
| `normalize.extra.dataKey` | 请求级选项：信封负载字段名 | 无法表达 | — | 缺口 |
| `normalize.extra.codeKey` | 请求级选项：信封业务码字段名 | 无法表达 | — | 缺口 |
| `normalize.extra.messageKey` | 请求级选项：信封消息字段名 | 无法表达 | — | 缺口 |
| `normalize.extra.isSuccess` | 请求级选项：业务成功判定回调 | 无法表达 | — | 缺口 |
| `normalize.extra.shouldNormalize` | 请求级选项：响应是否拆包的判定回调 | 无法表达 | — | 缺口 |
| `normalize.export.DiomanException` | 导出类型：公开类型 DiomanException | 无法表达 | — | 缺口 |
| `normalize.ctor.DiomanException.new` | 构造方法：构造 DiomanException.new | 无法表达 | — | 缺口 |
| `normalize.nested.DiomanException.code` | 嵌套字段：异常业务码 | 无法表达 | — | 缺口 |
| `normalize.nested.DiomanException.message` | 嵌套字段：异常消息 | 无法表达 | — | 缺口 |
| `normalize.nested.DiomanException.data` | 嵌套字段：异常负载 | 无法表达 | — | 缺口 |
| `normalize.method.DiomanException.toString` | 公开方法：undefined | 无法表达 | — | 缺口 |
| `normalize.export.DiomanNormalize` | 导出类型：公开类型 DiomanNormalize | 无法表达 | — | 缺口 |
| `normalize.ctor.DiomanNormalize.new` | 构造方法：构造 DiomanNormalize.new | 无法表达 | — | 缺口 |
| `normalize.opt.dataKey` | 构造选项：信封负载字段名 | 无法表达 | — | 缺口 |
| `normalize.opt.codeKey` | 构造选项：信封业务码字段名 | 无法表达 | — | 缺口 |
| `normalize.opt.messageKey` | 构造选项：信封消息字段名 | 无法表达 | — | 缺口 |
| `normalize.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `normalize.opt.isSuccess` | 构造选项：业务成功判定回调 | 无法表达 | — | 缺口 |
| `normalize.opt.shouldNormalize` | 构造选项：响应是否拆包的判定回调 | 无法表达 | — | 缺口 |
| `normalize.prop.DiomanNormalize.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `normalize.prop.DiomanNormalize.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `normalize.prop.DiomanNormalize.ordering` | 公开属性：插件声明的顺序与依赖约束 | 无法表达 | — | 缺口 |
| `normalize.method.DiomanNormalize.onResponse` | 公开方法：响应拦截入口 | 部分 | `normalize.envelope-unwrap`、`normalize.no-envelope-passthrough` | 已关联 |

### cache

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `cache.export.DiomanClonePolicy` | 导出枚举：公开枚举 DiomanClonePolicy | 无法表达 | — | 缺口 |
| `cache.enum.DiomanClonePolicy.none` | 枚举值：枚举取值 DiomanClonePolicy.none | 无法表达 | — | 缺口 |
| `cache.enum.DiomanClonePolicy.shallow` | 枚举值：枚举取值 DiomanClonePolicy.shallow | 无法表达 | — | 缺口 |
| `cache.enum.DiomanClonePolicy.deep` | 枚举值：枚举取值 DiomanClonePolicy.deep | 无法表达 | — | 缺口 |
| `cache.export.DiomanCachePolicy` | 导出枚举：公开枚举 DiomanCachePolicy | 无法表达 | — | 缺口 |
| `cache.enum.DiomanCachePolicy.none` | 枚举值：枚举取值 DiomanCachePolicy.none | 无法表达 | — | 缺口 |
| `cache.enum.DiomanCachePolicy.memo` | 枚举值：枚举取值 DiomanCachePolicy.memo | 无法表达 | — | 缺口 |
| `cache.enum.DiomanCachePolicy.persist` | 枚举值：缓存持久化端口 | 无法表达 | — | 缺口 |
| `cache.enum.DiomanCachePolicy.both` | 枚举值：枚举取值 DiomanCachePolicy.both | 无法表达 | — | 缺口 |
| `cache.export.DiomanCachePersist` | 导出类型：公开类型 DiomanCachePersist | 无法表达 | — | 缺口 |
| `cache.method.DiomanCachePersist.read` | 公开方法：读取持久化缓存条目 | 无法表达 | — | 缺口 |
| `cache.method.DiomanCachePersist.write` | 公开方法：写入持久化缓存条目 | 无法表达 | — | 缺口 |
| `cache.method.DiomanCachePersist.remove` | 公开方法：删除缓存条目、mock 路由或插件（以所属类型为准） | 无法表达 | — | 缺口 |
| `cache.method.DiomanCachePersist.erase` | 公开方法：清空持久化缓存 | 无法表达 | — | 缺口 |
| `cache.export.DiomanCacheOptions` | 导出类型：公开类型 DiomanCacheOptions | 无法表达 | — | 缺口 |
| `cache.ctor.DiomanCacheOptions.new` | 构造方法：构造 DiomanCacheOptions.new | 无法表达 | — | 缺口 |
| `cache.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 部分 | `cache.per-request-disable` | 已关联 |
| `cache.extra.expires` | 请求级选项：缓存有效期，毫秒 | 部分 | `cache.per-request-ttl` | 已关联 |
| `cache.extra.clone` | 请求级选项：缓存数据复制策略：none/shallow/deep | 无法表达 | — | 缺口 |
| `cache.extra.maxEntries` | 请求级选项：内存缓存容量；请求级字段已弃用且无效；已弃用，源码忽略该请求级容量；需显式兼容验证，不能与构造容量混算。 | 无法表达 | — | 缺口 |
| `cache.extra.shouldCache` | 请求级选项：请求是否可缓存的判定回调 | 无法表达 | — | 缺口 |
| `cache.extra.now` | 请求级选项：用于时间判断的可注入时钟 | 无法表达 | — | 缺口 |
| `cache.extra.cachePolicy` | 请求级选项：缓存策略：none/memo/persist/both | 无法表达 | — | 缺口 |
| `cache.export.DiomanCache` | 导出类型：公开类型 DiomanCache | 无法表达 | — | 缺口 |
| `cache.ctor.DiomanCache.new` | 构造方法：构造 DiomanCache.new | 无法表达 | — | 缺口 |
| `cache.opt.persist` | 构造选项：缓存持久化端口 | 无法表达 | — | 缺口 |
| `cache.opt.cachePolicy` | 构造选项：缓存策略：none/memo/persist/both | 无法表达 | — | 缺口 |
| `cache.opt.expires` | 构造选项：缓存有效期，毫秒 | 部分 | `cache.hit-no-server`、`cache.ttl-expiry` | 已关联 |
| `cache.opt.clone` | 构造选项：缓存数据复制策略：none/shallow/deep | 无法表达 | — | 缺口 |
| `cache.opt.maxEntries` | 构造选项：内存缓存容量；达到上限按缓存策略淘汰 | 部分 | `cache.max-entries-evicts` | 已关联 |
| `cache.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `cache.opt.shouldCache` | 构造选项：请求是否可缓存的判定回调 | 部分 | `cache.diff-body-no-mixup`、`cache.post-not-cached-by-default` | 已关联 |
| `cache.opt.now` | 构造选项：用于时间判断的可注入时钟 | 无法表达 | — | 缺口 |
| `cache.prop.DiomanCache.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `cache.prop.DiomanCache.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `cache.prop.DiomanCache.ordering` | 公开属性：插件声明的顺序与依赖约束 | 无法表达 | — | 缺口 |
| `cache.method.DiomanCache.onRequest` | 公开方法：请求拦截入口 | 部分 | `cache.hit-no-server`、`cache.ttl-expiry` | 已关联 |
| `cache.method.DiomanCache.onResponse` | 公开方法：响应拦截入口 | 部分 | `cache.failure-then-success-cached`、`cache.http-error-not-cached` | 已关联 |
| `cache.method.DiomanCache.remove` | 公开方法：删除缓存条目、mock 路由或插件（以所属类型为准） | 无法表达 | — | 缺口 |
| `cache.method.DiomanCache.clear` | 公开方法：清空缓存或令牌（以所属类型为准） | 部分 | `cache.clear` | 已关联 |
| `cache.method.DiomanCache.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |
| `cache.nested.persist.data` | 嵌套字段：持久化条目的 data 字段 | 无法表达 | — | 缺口 |
| `cache.nested.persist.expiresAt` | 嵌套字段：持久化条目的 expiresAt 字段 | 无法表达 | — | 缺口 |

### offline

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `offline.export.DiomanOfflineReason` | 导出枚举：公开枚举 DiomanOfflineReason | 无法表达 | — | 缺口 |
| `offline.enum.DiomanOfflineReason.queueFull` | 枚举值：枚举取值 DiomanOfflineReason.queueFull | 无法表达 | — | 缺口 |
| `offline.enum.DiomanOfflineReason.timeout` | 枚举值：枚举取值 DiomanOfflineReason.timeout | 无法表达 | — | 缺口 |
| `offline.enum.DiomanOfflineReason.disposed` | 枚举值：枚举取值 DiomanOfflineReason.disposed | 无法表达 | — | 缺口 |
| `offline.export.DiomanOfflineException` | 导出类型：公开类型 DiomanOfflineException | 无法表达 | — | 缺口 |
| `offline.ctor.DiomanOfflineException.new` | 构造方法：构造 DiomanOfflineException.new | 无法表达 | — | 缺口 |
| `offline.nested.DiomanOfflineException.reason` | 嵌套字段：离线异常原因 | 无法表达 | — | 缺口 |
| `offline.method.DiomanOfflineException.toString` | 公开方法：undefined | 无法表达 | — | 缺口 |
| `offline.export.DiomanOfflineOptions` | 导出类型：公开类型 DiomanOfflineOptions | 无法表达 | — | 缺口 |
| `offline.ctor.DiomanOfflineOptions.new` | 构造方法：构造 DiomanOfflineOptions.new | 无法表达 | — | 缺口 |
| `offline.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `offline.extra.shouldQueue` | 请求级选项：请求是否应进入离线队列的回调 | 无法表达 | — | 缺口 |
| `offline.extra.maxQueueSize` | 请求级选项：离线队列上限；满时淘汰最旧请求 | 无法表达 | — | 缺口 |
| `offline.extra.maxWait` | 请求级选项：离线最长等待；空值不限，零值快速失败 | 无法表达 | — | 缺口 |
| `offline.export.DiomanOffline` | 导出类型：公开类型 DiomanOffline | 无法表达 | — | 缺口 |
| `offline.ctor.DiomanOffline.new` | 构造方法：构造 DiomanOffline.new | 无法表达 | — | 缺口 |
| `offline.opt.isOnline` | 构造选项：当前是否在线的同步回调 | 部分 | `offline.probe-fails-fast`、`offline.online-passes` | 已关联 |
| `offline.opt.onConnectivityChanged` | 构造选项：在线状态事件流；true 触发队列冲刷；宿主有事件输入，但 maxWait=0 无待恢复队列；未证明事件冲刷语义。 | 部分 | — | 存疑 |
| `offline.opt.shouldQueue` | 构造选项：请求是否应进入离线队列的回调 | 无法表达 | — | 缺口 |
| `offline.opt.maxQueueSize` | 构造选项：离线队列上限；满时淘汰最旧请求 | 无法表达 | — | 缺口 |
| `offline.opt.maxWait` | 构造选项：离线最长等待；空值不限，零值快速失败 | 部分 | `offline.probe-fails-fast`、`offline.parallel-all-offline` | 已关联 |
| `offline.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `offline.prop.DiomanOffline.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `offline.prop.DiomanOffline.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `offline.prop.DiomanOffline.ordering` | 公开属性：插件声明的顺序与依赖约束 | 无法表达 | — | 缺口 |
| `offline.prop.DiomanOffline.pending` | 公开属性：当前离线队列长度；与宿主 pending_after 不同 | 无法表达 | — | 缺口 |
| `offline.method.DiomanOffline.onRequest` | 公开方法：请求拦截入口 | 部分 | `offline.probe-fails-fast`、`offline.online-passes` | 已关联 |
| `offline.method.DiomanOffline.flush` | 公开方法：恢复全部离线排队请求；取消者保持取消 | 无法表达 | — | 缺口 |
| `offline.method.DiomanOffline.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |

### share

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `share.export.DiomanSharePolicy` | 导出枚举：公开枚举 DiomanSharePolicy | 无法表达 | — | 缺口 |
| `share.enum.DiomanSharePolicy.start` | 枚举值：枚举取值 DiomanSharePolicy.start | 无法表达 | — | 缺口 |
| `share.enum.DiomanSharePolicy.end` | 枚举值：枚举取值 DiomanSharePolicy.end | 无法表达 | — | 缺口 |
| `share.enum.DiomanSharePolicy.race` | 枚举值：枚举取值 DiomanSharePolicy.race | 无法表达 | — | 缺口 |
| `share.enum.DiomanSharePolicy.retry` | 枚举值：枚举取值 DiomanSharePolicy.retry | 无法表达 | — | 缺口 |
| `share.enum.DiomanSharePolicy.none` | 枚举值：枚举取值 DiomanSharePolicy.none | 无法表达 | — | 缺口 |
| `share.export.DiomanShareOptions` | 导出类型：公开类型 DiomanShareOptions | 无法表达 | — | 缺口 |
| `share.ctor.DiomanShareOptions.new` | 构造方法：构造 DiomanShareOptions.new | 无法表达 | — | 缺口 |
| `share.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 部分 | `share.per-request-disable` | 已关联 |
| `share.extra.policy` | 请求级选项：共享策略：start/end/race/retry/none | 部分 | `share.end-last-wins`、`share.race-first-success`、`share.none-policy` | 已关联 |
| `share.extra.retries` | 请求级选项：share 的 retry 策略重试预算 | 无法表达 | — | 缺口 |
| `share.extra.interval` | 请求级选项：share 的 retry 策略重试间隔 | 无法表达 | — | 缺口 |
| `share.export.DiomanShare` | 导出类型：公开类型 DiomanShare | 无法表达 | — | 缺口 |
| `share.ctor.DiomanShare.new` | 构造方法：构造 DiomanShare.new | 无法表达 | — | 缺口 |
| `share.opt.policy` | 构造选项：共享策略：start/end/race/retry/none | 部分 | `share.start-single-flight`、`share.end-last-wins`、`share.race-first-success`、`share.race-first-fails-second-wins` | 已关联 |
| `share.opt.retries` | 构造选项：share 的 retry 策略重试预算 | 无法表达 | — | 缺口 |
| `share.opt.interval` | 构造选项：share 的 retry 策略重试间隔 | 无法表达 | — | 缺口 |
| `share.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `share.setter.DiomanShare.dio` | 公开绑定：绑定或解绑宿主 Dio | 无法表达 | — | 缺口 |
| `share.method.DiomanShare.registerDownstreamSettler` | 公开方法：旧结算者兼容计数；已弃用，不改变结算行为 | 无法表达 | — | 缺口 |
| `share.prop.DiomanShare.hasMultipleDownstreamSettlers` | 公开属性：旧结算者兼容计数属性；不改变行为 | 无法表达 | — | 缺口 |
| `share.method.DiomanShare.settle` | 公开方法：显式结算指定键的共享请求，可传 response/error | 无法表达 | — | 缺口 |
| `share.prop.DiomanShare.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `share.prop.DiomanShare.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `share.method.DiomanShare.onRequest` | 公开方法：请求拦截入口 | 部分 | `share.start-single-flight`、`share.end-last-wins`、`share.race-first-success` | 已关联 |
| `share.method.DiomanShare.onResponse` | 公开方法：响应拦截入口 | 部分 | `share.start-single-flight`、`share.race-first-success` | 已关联 |
| `share.method.DiomanShare.onError` | 公开方法：错误拦截入口 | 部分 | `share.leader-failure-all-fail`、`share.reuse-after-failure` | 已关联 |
| `share.method.DiomanShare.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |

### mock

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `mock.export.MockFallbackDecider` | 回调类型：公开回调签名 MockFallbackDecider | 无法表达 | — | 缺口 |
| `mock.function.defaultFallback` | 导出函数：公开函数 defaultFallback | 无法表达 | — | 缺口 |
| `mock.export.DiomanMockOptions` | 导出类型：公开类型 DiomanMockOptions | 无法表达 | — | 缺口 |
| `mock.ctor.DiomanMockOptions.new` | 构造方法：构造 DiomanMockOptions.new | 无法表达 | — | 缺口 |
| `mock.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 部分 | `mock.routes-to-mock-server`、`mock.per-request-off-goes-real`、`mock.mixed-parallel` | 已关联 |
| `mock.extra.mockUrl` | 请求级选项：远程 mock 基址 | 无法表达 | — | 缺口 |
| `mock.extra.routes` | 请求级选项：本地 mock 路由表：路由键到 MockHandler 回调 | 无法表达 | — | 缺口 |
| `mock.extra.fallbackWhen` | 请求级选项：远程 mock 失败时是否回退真实接口的回调 | 无法表达 | — | 缺口 |
| `mock.export.DiomanMock` | 导出类型：公开类型 DiomanMock | 无法表达 | — | 缺口 |
| `mock.ctor.DiomanMock.new` | 构造方法：构造 DiomanMock.new | 无法表达 | — | 缺口 |
| `mock.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `mock.opt.mockUrl` | 构造选项：远程 mock 基址 | 部分 | `mock.routes-to-mock-server`、`mock.query-and-body-forwarded` | 已关联 |
| `mock.opt.fallbackWhen` | 构造选项：远程 mock 失败时是否回退真实接口的回调 | 部分 | `mock.fallback-to-real-on-404` | 已关联 |
| `mock.opt.routes` | 构造选项：本地 mock 路由表：路由键到 MockHandler 回调 | 无法表达 | — | 缺口 |
| `mock.prop.DiomanMock.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `mock.prop.DiomanMock.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `mock.method.DiomanMock.add` | 公开方法：注册本地 mock 路由回调 | 无法表达 | — | 缺口 |
| `mock.method.DiomanMock.remove` | 公开方法：删除缓存条目、mock 路由或插件（以所属类型为准） | 无法表达 | — | 缺口 |
| `mock.method.DiomanMock.reset` | 公开方法：重置 mock 路由或全部熔断桶 | 无法表达 | — | 缺口 |
| `mock.method.DiomanMock.onRequest` | 公开方法：请求拦截入口 | 部分 | `mock.routes-to-mock-server`、`mock.fallback-to-real-on-404` | 已关联 |
| `mock.method.DiomanMock.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |
| `mock.export.MockHandler` | 回调类型：公开回调签名 MockHandler | 无法表达 | — | 缺口 |

### cancel

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `cancel.export.DiomanCancelOptions` | 导出类型：公开类型 DiomanCancelOptions | 无法表达 | — | 缺口 |
| `cancel.ctor.DiomanCancelOptions.new` | 构造方法：构造 DiomanCancelOptions.new | 无法表达 | — | 缺口 |
| `cancel.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `cancel.export.DiomanCancel` | 导出类型：公开类型 DiomanCancel | 无法表达 | — | 缺口 |
| `cancel.ctor.DiomanCancel.new` | 构造方法：构造 DiomanCancel.new | 无法表达 | — | 缺口 |
| `cancel.prop.DiomanCancel.config` | 公开属性：解析后的构造配置 | 无法表达 | — | 缺口 |
| `cancel.prop.DiomanCancel.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `cancel.prop.DiomanCancel.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `cancel.prop.DiomanCancel.ordering` | 公开属性：插件声明的顺序与依赖约束 | 无法表达 | — | 缺口 |
| `cancel.method.DiomanCancel.onRequest` | 公开方法：请求拦截入口 | 无法表达 | — | 缺口 |
| `cancel.method.DiomanCancel.onResponse` | 公开方法：响应拦截入口 | 无法表达 | — | 缺口 |
| `cancel.method.DiomanCancel.onError` | 公开方法：错误拦截入口 | 无法表达 | — | 缺口 |
| `cancel.method.DiomanCancel.track` | 公开方法：纳入外部 CancelToken | 部分 | `cancel.all`、`cancel.all-then-new-works`、`cancel.double-cancel` | 已关联 |
| `cancel.method.DiomanCancel.untrack` | 公开方法：移除外部 CancelToken | 部分 | `cancel.all`、`cancel.all-then-new-works`、`cancel.double-cancel` | 已关联 |
| `cancel.method.DiomanCancel.cancelAll` | 公开方法：取消已跟踪请求，可传原因；返回取消数量 | 部分 | `cancel.all`、`cancel.all-then-new-works`、`cancel.double-cancel` | 已关联 |
| `cancel.method.DiomanCancel.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |
| `cancel.function.cancelAll` | 导出函数：取消已跟踪请求，可传原因；返回取消数量 | 无法表达 | — | 缺口 |

### loading

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `loading.export.DiomanLoadingOptions` | 导出类型：公开类型 DiomanLoadingOptions | 无法表达 | — | 缺口 |
| `loading.ctor.DiomanLoadingOptions.new` | 构造方法：构造 DiomanLoadingOptions.new | 无法表达 | — | 缺口 |
| `loading.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 部分 | `loading.per-request-off`、`loading.mixed-silent-and-normal` | 已关联 |
| `loading.extra.onChanged` | 请求级选项：加载状态 true/false 回调 | 无法表达 | — | 缺口 |
| `loading.extra.delay` | 请求级选项：显示加载状态前的延迟 | 无法表达 | — | 缺口 |
| `loading.extra.delayClose` | 请求级选项：延迟关闭加载状态 | 无法表达 | — | 缺口 |
| `loading.export.DiomanLoading` | 导出类型：公开类型 DiomanLoading | 无法表达 | — | 缺口 |
| `loading.ctor.DiomanLoading.new` | 构造方法：构造 DiomanLoading.new | 无法表达 | — | 缺口 |
| `loading.opt.onChanged` | 构造选项：加载状态 true/false 回调 | 部分 | [G2](#g2) | 已关联 |
| `loading.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `loading.opt.delay` | 构造选项：显示加载状态前的延迟 | 部分 | `combo.loading-cache-hit-delay`、`loading.delay-fast-never-shows`、`loading.delay-slow-shows` | 已关联 |
| `loading.opt.delayClose` | 构造选项：延迟关闭加载状态 | 部分 | `loading.delay-close-merges` | 已关联 |
| `loading.prop.DiomanLoading.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `loading.prop.DiomanLoading.activeCount` | 公开属性：当前加载计数；与宿主 pending_after 不同 | 无法表达 | — | 缺口 |
| `loading.prop.DiomanLoading.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `loading.prop.DiomanLoading.ordering` | 公开属性：插件声明的顺序与依赖约束 | 无法表达 | — | 缺口 |
| `loading.method.DiomanLoading.onRequest` | 公开方法：请求拦截入口 | 部分 | `loading.single-pair`、`loading.concurrent-single-pair` | 已关联 |
| `loading.method.DiomanLoading.onResponse` | 公开方法：响应拦截入口 | 部分 | `loading.single-pair`、`loading.delay-close-merges` | 已关联 |
| `loading.method.DiomanLoading.onError` | 公开方法：错误拦截入口 | 部分 | `loading.failure-closes`、`loading.cancel-closes`、`loading.timeout-closes` | 已关联 |
| `loading.method.DiomanLoading.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |

### auth

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `auth.export.DiomanTokenManager` | 导出类型：公开类型 DiomanTokenManager | 无法表达 | — | 缺口 |
| `auth.prop.DiomanTokenManager.accessToken` | 公开属性：读取 DiomanTokenManager.accessToken | 无法表达 | — | 缺口 |
| `auth.prop.DiomanTokenManager.refreshToken` | 公开属性：读取 DiomanTokenManager.refreshToken | 无法表达 | — | 缺口 |
| `auth.prop.DiomanTokenManager.canRefresh` | 公开属性：读取 DiomanTokenManager.canRefresh | 无法表达 | — | 缺口 |
| `auth.method.DiomanTokenManager.clear` | 公开方法：清空缓存或令牌（以所属类型为准） | 无法表达 | — | 缺口 |
| `auth.export.DiomanAuthFailureAction` | 导出枚举：公开枚举 DiomanAuthFailureAction | 无法表达 | — | 缺口 |
| `auth.enum.DiomanAuthFailureAction.refresh` | 枚举值：枚举取值 DiomanAuthFailureAction.refresh | 无法表达 | — | 缺口 |
| `auth.enum.DiomanAuthFailureAction.replay` | 枚举值：枚举取值 DiomanAuthFailureAction.replay | 无法表达 | — | 缺口 |
| `auth.enum.DiomanAuthFailureAction.deny` | 枚举值：枚举取值 DiomanAuthFailureAction.deny | 无法表达 | — | 缺口 |
| `auth.enum.DiomanAuthFailureAction.expired` | 枚举值：枚举取值 DiomanAuthFailureAction.expired | 无法表达 | — | 缺口 |
| `auth.enum.DiomanAuthFailureAction.others` | 枚举值：枚举取值 DiomanAuthFailureAction.others | 无法表达 | — | 缺口 |
| `auth.function.defaultAuthFailure` | 导出函数：公开函数 defaultAuthFailure | 无法表达 | — | 缺口 |
| `auth.export.DiomanAuthOptions` | 导出类型：公开类型 DiomanAuthOptions | 无法表达 | — | 缺口 |
| `auth.ctor.DiomanAuthOptions.new` | 构造方法：构造 DiomanAuthOptions.new | 无法表达 | — | 缺口 |
| `auth.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 部分 | `auth.code-only-401-refresh-once-then-expired`、`auth.code-only-401-recovers`、`auth.code-only-401-no-refresh-token`、`auth.code-only-401-concurrent`、`combo.all-seventeen-plugins` | 已关联 |
| `auth.extra.onAccessDenied` | 请求级选项：访问被拒绝异步回调 | 无法表达 | — | 缺口 |
| `auth.extra.onAccessExpired` | 请求级选项：会话过期异步回调 | 无法表达 | — | 缺口 |
| `auth.extra.onFailure` | 请求级选项：鉴权失败动作判定回调 | 无法表达 | — | 缺口 |
| `auth.extra.ready` | 请求级选项：自定义异步令牌注入回调 | 无法表达 | — | 缺口 |
| `auth.extra.isProtected` | 请求级选项：受保护请求判定回调 | 无法表达 | — | 缺口 |
| `auth.extra.expiresAt` | 请求级选项：由令牌取得到期时间的回调 | 无法表达 | — | 缺口 |
| `auth.extra.refreshLeeway` | 请求级选项：主动刷新提前量 | 无法表达 | — | 缺口 |
| `auth.extra.now` | 请求级选项：用于时间判断的可注入时钟 | 无法表达 | — | 缺口 |
| `auth.extra.headerKey` | 请求级选项：鉴权头键名 | 无法表达 | — | 缺口 |
| `auth.extra.buildHeader` | 请求级选项：令牌到鉴权头值的转换回调 | 无法表达 | — | 缺口 |
| `auth.export.DiomanAuth` | 导出类型：公开类型 DiomanAuth | 无法表达 | — | 缺口 |
| `auth.ctor.DiomanAuth.new` | 构造方法：构造 DiomanAuth.new | 无法表达 | — | 缺口 |
| `auth.opt.tokenManager` | 构造选项：令牌存储端口：accessToken/refreshToken/canRefresh/clear | 无法表达 | — | 缺口 |
| `auth.opt.onRefresh` | 构造选项：刷新令牌的异步回调 | 部分 | [G3](#g3) | 已关联 |
| `auth.opt.onAccessExpired` | 构造选项：会话过期异步回调 | 部分 | [G4](#g4) | 已关联 |
| `auth.opt.onAccessDenied` | 构造选项：访问被拒绝异步回调 | 部分 | [G5](#g5) | 已关联 |
| `auth.opt.onFailure` | 构造选项：鉴权失败动作判定回调 | 部分 | `auth.expired-refresh-replay`、`auth.deny-403`、`auth.no-refresh-user-expired` | 已关联 |
| `auth.opt.ready` | 构造选项：自定义异步令牌注入回调 | 无法表达 | — | 缺口 |
| `auth.opt.isProtected` | 构造选项：受保护请求判定回调 | 部分 | `auth.anon-and-private-mix`、`auth.public-401-no-refresh`、`auth.public-ignores-expired-token` | 已关联 |
| `auth.opt.expiresAt` | 构造选项：由令牌取得到期时间的回调 | 无法表达 | — | 缺口 |
| `auth.opt.refreshLeeway` | 构造选项：主动刷新提前量 | 无法表达 | — | 缺口 |
| `auth.opt.now` | 构造选项：用于时间判断的可注入时钟 | 无法表达 | — | 缺口 |
| `auth.opt.headerKey` | 构造选项：鉴权头键名 | 无法表达 | — | 缺口 |
| `auth.opt.buildHeader` | 构造选项：令牌到鉴权头值的转换回调 | 部分 | `auth.anon-and-private-mix`、`auth.relogin-other-user` | 已关联 |
| `auth.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `auth.setter.DiomanAuth.cancel` | 公开绑定：绑定 DiomanAuth.cancel | 无法表达 | — | 缺口 |
| `auth.setter.DiomanAuth.share` | 公开绑定：绑定 DiomanAuth.share | 无法表达 | — | 缺口 |
| `auth.setter.DiomanAuth.dio` | 公开绑定：绑定或解绑宿主 Dio | 无法表达 | — | 缺口 |
| `auth.prop.DiomanAuth.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `auth.prop.DiomanAuth.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `auth.method.DiomanAuth.onRequest` | 公开方法：请求拦截入口 | 部分 | `auth.anon-and-private-mix`、`auth.token-none-refresh-none` | 已关联 |
| `auth.method.DiomanAuth.onResponse` | 公开方法：响应拦截入口 | 部分 | `auth.anon-and-private-mix` | 已关联 |
| `auth.method.DiomanAuth.onError` | 公开方法：错误拦截入口 | 部分 | `auth.expired-refresh-replay`、`auth.deny-403` | 已关联 |
| `auth.method.DiomanAuth.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |

### retry

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `retry.export.DiomanJitter` | 回调类型：公开回调签名 DiomanJitter | 无法表达 | — | 缺口 |
| `retry.export.DiomanRetryDelay` | 回调类型：公开回调签名 DiomanRetryDelay | 无法表达 | — | 缺口 |
| `retry.export.DiomanShouldRetry` | 回调类型：公开回调签名 DiomanShouldRetry | 无法表达 | — | 缺口 |
| `retry.export.DiomanRetryOptions` | 导出类型：公开类型 DiomanRetryOptions | 无法表达 | — | 缺口 |
| `retry.ctor.DiomanRetryOptions.new` | 构造方法：构造 DiomanRetryOptions.new | 无法表达 | — | 缺口 |
| `retry.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 部分 | `retry.per-request-disable` | 已关联 |
| `retry.extra.max` | 请求级选项：最大重试次数 | 部分 | `retry.500-then-ok`、`retry.exhausted`、`retry.per-request-max` | 已关联 |
| `retry.extra.methods` | 请求级选项：允许重试的方法白名单 | 部分 | `retry.post-default-no-retry`、`retry.post-opt-in-body-intact`、`retry.put-delete-retried` | 已关联 |
| `retry.extra.shouldRetry` | 请求级选项：可空布尔重试判定回调；空值退回默认判定 | 部分 | `retry.business-code-not-retried`、`retry.custom-status-list`、`retry.parse-error-not-retried`、`retry.network-error-retried` | 已关联 |
| `retry.extra.statusCodes` | 请求级选项：原生 HTTP 重试状态码列表；与宿主逻辑状态回调不同 | 无法表达 | — | 缺口 |
| `retry.extra.delay` | 请求级选项：按次数、配置、响应、错误返回退避时长的回调 | 部分 | `retry.delay-respected` | 已关联 |
| `retry.extra.jitter` | 请求级选项：抖动配置：布尔值或 DiomanJitter 回调 | 无法表达 | — | 缺口 |
| `retry.extra.delayMax` | 请求级选项：重试退避上限 | 无法表达 | — | 缺口 |
| `retry.extra.respectRetryAfter` | 请求级选项：是否采用 Retry-After | 部分 | `retry.503-retry-after`、`retry.respects-retry-after-over-delay` | 已关联 |
| `retry.extra.afterStatusCodes` | 请求级选项：应用 Retry-After 的状态码列表 | 无法表达 | — | 缺口 |
| `retry.extra.retryAfterMax` | 请求级选项：Retry-After 等待上限 | 无法表达 | — | 缺口 |
| `retry.export.DiomanRetry` | 导出类型：公开类型 DiomanRetry | 无法表达 | — | 缺口 |
| `retry.ctor.DiomanRetry.new` | 构造方法：构造 DiomanRetry.new | 无法表达 | — | 缺口 |
| `retry.opt.max` | 构造选项：最大重试次数；全局配置同时写入 DiomanRetryOptions；构造值被遮蔽，保守不计覆盖。 | 部分 | — | 存疑 |
| `retry.opt.methods` | 构造选项：允许重试的方法白名单；全局配置同时写入 DiomanRetryOptions；构造值被遮蔽，保守不计覆盖。 | 部分 | — | 存疑 |
| `retry.opt.shouldRetry` | 构造选项：可空布尔重试判定回调；空值退回默认判定；全局配置同时写入 DiomanRetryOptions；构造值被遮蔽，保守不计覆盖。 | 部分 | — | 存疑 |
| `retry.opt.statusCodes` | 构造选项：原生 HTTP 重试状态码列表；与宿主逻辑状态回调不同 | 无法表达 | — | 缺口 |
| `retry.opt.delay` | 构造选项：按次数、配置、响应、错误返回退避时长的回调；全局配置同时写入 DiomanRetryOptions；构造值被遮蔽，保守不计覆盖。 | 部分 | — | 存疑 |
| `retry.opt.jitter` | 构造选项：抖动配置：布尔值或 DiomanJitter 回调 | 无法表达 | — | 缺口 |
| `retry.opt.delayMax` | 构造选项：按次数、配置、响应、错误返回退避时长的回调 | 无法表达 | — | 缺口 |
| `retry.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `retry.opt.respectRetryAfter` | 构造选项：是否采用 Retry-After；全局配置同时写入 DiomanRetryOptions；构造值被遮蔽，保守不计覆盖。 | 部分 | — | 存疑 |
| `retry.opt.afterStatusCodes` | 构造选项：应用 Retry-After 的状态码列表 | 无法表达 | — | 缺口 |
| `retry.opt.retryAfterMax` | 构造选项：Retry-After 等待上限 | 无法表达 | — | 缺口 |
| `retry.opt.onRetry` | 构造选项：每次重试的 attempt 回调 | 无法表达 | — | 缺口 |
| `retry.setter.DiomanRetry.dio` | 公开绑定：绑定或解绑宿主 Dio | 无法表达 | — | 缺口 |
| `retry.setter.DiomanRetry.share` | 公开绑定：绑定 DiomanRetry.share | 无法表达 | — | 缺口 |
| `retry.setter.DiomanRetry.cancel` | 公开绑定：绑定 DiomanRetry.cancel | 无法表达 | — | 缺口 |
| `retry.setter.DiomanRetry.breaker` | 公开绑定：绑定 DiomanRetry.breaker | 无法表达 | — | 缺口 |
| `retry.prop.DiomanRetry.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `retry.prop.DiomanRetry.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `retry.method.DiomanRetry.onRequest` | 公开方法：请求拦截入口 | 无法表达 | — | 缺口 |
| `retry.method.DiomanRetry.onResponse` | 公开方法：响应拦截入口 | 无法表达 | — | 缺口 |
| `retry.method.DiomanRetry.onError` | 公开方法：错误拦截入口 | 部分 | `retry.500-then-ok`、`retry.exhausted`、`retry.network-error-retried` | 已关联 |
| `retry.method.DiomanRetry.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |
| `retry.extra.rawFalse` | 嵌套字段：extra['dioman:retry']=false 的原生简写 | 无法表达 | — | 缺口 |
| `retry.extra.rawInt` | 嵌套字段：extra['dioman:retry']=整数的原生简写 | 无法表达 | — | 缺口 |

### breaker

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `breaker.export.DiomanBreakerState` | 导出枚举：公开枚举 DiomanBreakerState | 无法表达 | — | 缺口 |
| `breaker.enum.DiomanBreakerState.closed` | 枚举值：枚举取值 DiomanBreakerState.closed | 无法表达 | — | 缺口 |
| `breaker.enum.DiomanBreakerState.open` | 枚举值：枚举取值 DiomanBreakerState.open | 无法表达 | — | 缺口 |
| `breaker.enum.DiomanBreakerState.halfOpen` | 枚举值：枚举取值 DiomanBreakerState.halfOpen | 无法表达 | — | 缺口 |
| `breaker.export.DiomanShouldTrip` | 回调类型：公开回调签名 DiomanShouldTrip | 无法表达 | — | 缺口 |
| `breaker.export.DiomanBreakerOpenException` | 导出类型：公开类型 DiomanBreakerOpenException | 无法表达 | — | 缺口 |
| `breaker.ctor.DiomanBreakerOpenException.new` | 构造方法：构造 DiomanBreakerOpenException.new | 无法表达 | — | 缺口 |
| `breaker.nested.DiomanBreakerOpenException.bucketKey` | 嵌套字段：熔断异常关联桶键 | 无法表达 | — | 缺口 |
| `breaker.nested.DiomanBreakerOpenException.retryableAt` | 嵌套字段：熔断异常可重试时间 | 无法表达 | — | 缺口 |
| `breaker.method.DiomanBreakerOpenException.toString` | 公开方法：undefined | 无法表达 | — | 缺口 |
| `breaker.export.DiomanBreakerOptions` | 导出类型：公开类型 DiomanBreakerOptions | 无法表达 | — | 缺口 |
| `breaker.ctor.DiomanBreakerOptions.new` | 构造方法：构造 DiomanBreakerOptions.new | 无法表达 | — | 缺口 |
| `breaker.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 部分 | `breaker.per-request-skip` | 已关联 |
| `breaker.extra.failureThreshold` | 请求级选项：连续失败熔断阈值 | 无法表达 | — | 缺口 |
| `breaker.extra.resetDuration` | 请求级选项：熔断冷却时间 | 无法表达 | — | 缺口 |
| `breaker.extra.halfOpenMaxCalls` | 请求级选项：半开并行探测上限 | 无法表达 | — | 缺口 |
| `breaker.extra.shouldTrip` | 请求级选项：是否将响应或错误计为熔断失败的回调 | 无法表达 | — | 缺口 |
| `breaker.extra.keyBuilder` | 请求级选项：自定义熔断桶键回调 | 无法表达 | — | 缺口 |
| `breaker.export.DiomanBreaker` | 导出类型：公开类型 DiomanBreaker | 无法表达 | — | 缺口 |
| `breaker.ctor.DiomanBreaker.new` | 构造方法：构造 DiomanBreaker.new | 无法表达 | — | 缺口 |
| `breaker.opt.failureThreshold` | 构造选项：连续失败熔断阈值 | 部分 | `breaker.open-after-threshold`、`breaker.success-resets-count`、`breaker.parallel-trip` | 已关联 |
| `breaker.opt.resetDuration` | 构造选项：熔断冷却时间 | 部分 | `breaker.half-open-recovers`、`breaker.half-open-fail-reopens` | 已关联 |
| `breaker.opt.halfOpenMaxCalls` | 构造选项：半开并行探测上限 | 无法表达 | — | 缺口 |
| `breaker.opt.shouldTrip` | 构造选项：是否将响应或错误计为熔断失败的回调 | 部分 | `breaker.timeouts-count`、`breaker.4xx-not-counted`、`breaker.network-errors-trip` | 已关联 |
| `breaker.opt.keyBuilder` | 构造选项：自定义熔断桶键回调 | 无法表达 | — | 缺口 |
| `breaker.opt.onStateChange` | 构造选项：桶键及熔断状态 from/to 变化回调 | 无法表达 | — | 缺口 |
| `breaker.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `breaker.opt.idleTtl` | 构造选项：空闲熔断桶回收时间 | 无法表达 | — | 缺口 |
| `breaker.opt.maxBuckets` | 构造选项：熔断桶容量 | 无法表达 | — | 缺口 |
| `breaker.opt.clock` | 构造选项：熔断器可注入时钟 | 无法表达 | — | 缺口 |
| `breaker.prop.DiomanBreaker.defaultIdleTtl` | 公开属性：读取 DiomanBreaker.defaultIdleTtl | 无法表达 | — | 缺口 |
| `breaker.prop.DiomanBreaker.defaultMaxBuckets` | 公开属性：读取 DiomanBreaker.defaultMaxBuckets | 无法表达 | — | 缺口 |
| `breaker.prop.DiomanBreaker.bucketCount` | 公开属性：当前熔断桶数量 | 无法表达 | — | 缺口 |
| `breaker.prop.DiomanBreaker.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `breaker.prop.DiomanBreaker.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `breaker.prop.DiomanBreaker.ordering` | 公开属性：插件声明的顺序与依赖约束 | 无法表达 | — | 缺口 |
| `breaker.method.DiomanBreaker.isOpen` | 公开方法：查询请求所用桶是否拒绝放行 | 无法表达 | — | 缺口 |
| `breaker.method.DiomanBreaker.stateOf` | 公开方法：查询请求对应熔断状态 | 无法表达 | — | 缺口 |
| `breaker.method.DiomanBreaker.onRequest` | 公开方法：请求拦截入口 | 部分 | `breaker.open-after-threshold`、`breaker.per-request-skip` | 已关联 |
| `breaker.method.DiomanBreaker.onResponse` | 公开方法：响应拦截入口 | 部分 | `breaker.success-resets-count`、`breaker.half-open-recovers` | 已关联 |
| `breaker.method.DiomanBreaker.onError` | 公开方法：错误拦截入口 | 部分 | `breaker.half-open-fail-reopens`、`breaker.timeouts-count` | 已关联 |
| `breaker.method.DiomanBreaker.reset` | 公开方法：重置 mock 路由或全部熔断桶 | 无法表达 | — | 缺口 |
| `breaker.method.DiomanBreaker.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |

### log

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `log.export.LogWriter` | 回调类型：公开回调签名 LogWriter | 无法表达 | — | 缺口 |
| `log.export.DiomanLogOptions` | 导出类型：公开类型 DiomanLogOptions | 无法表达 | — | 缺口 |
| `log.ctor.DiomanLogOptions.new` | 构造方法：构造 DiomanLogOptions.new | 无法表达 | — | 缺口 |
| `log.extra.enabled` | 请求级选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `log.extra.logRequest` | 请求级选项：是否记录请求 | 无法表达 | — | 缺口 |
| `log.extra.logResponse` | 请求级选项：是否记录响应 | 无法表达 | — | 缺口 |
| `log.extra.logError` | 请求级选项：是否记录错误 | 无法表达 | — | 缺口 |
| `log.extra.logHeaders` | 请求级选项：是否记录头部 | 无法表达 | — | 缺口 |
| `log.extra.logBody` | 请求级选项：是否记录请求或响应体 | 无法表达 | — | 缺口 |
| `log.extra.maxBodyLength` | 请求级选项：日志正文截断上限 | 无法表达 | — | 缺口 |
| `log.extra.writer` | 请求级选项：日志输出回调，含可选 error 参数 | 无法表达 | — | 缺口 |
| `log.export.DiomanLog` | 导出类型：公开类型 DiomanLog | 无法表达 | — | 缺口 |
| `log.ctor.DiomanLog.new` | 构造方法：构造 DiomanLog.new | 无法表达 | — | 缺口 |
| `log.opt.logRequest` | 构造选项：是否记录请求 | 无法表达 | — | 缺口 |
| `log.opt.logResponse` | 构造选项：是否记录响应 | 无法表达 | — | 缺口 |
| `log.opt.logError` | 构造选项：是否记录错误 | 无法表达 | — | 缺口 |
| `log.opt.logHeaders` | 构造选项：是否记录头部 | 无法表达 | — | 缺口 |
| `log.opt.logBody` | 构造选项：是否记录请求或响应体 | 无法表达 | — | 缺口 |
| `log.opt.maxBodyLength` | 构造选项：日志正文截断上限 | 无法表达 | — | 缺口 |
| `log.opt.enabled` | 构造选项：启用开关（空值继承插件配置） | 无法表达 | — | 缺口 |
| `log.opt.writer` | 构造选项：日志输出回调，含可选 error 参数 | 无法表达 | — | 缺口 |
| `log.prop.DiomanLog.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `log.prop.DiomanLog.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `log.method.DiomanLog.onRequest` | 公开方法：请求拦截入口 | 无法表达 | — | 缺口 |
| `log.method.DiomanLog.onResponse` | 公开方法：响应拦截入口 | 无法表达 | — | 缺口 |
| `log.method.DiomanLog.onError` | 公开方法：错误拦截入口 | 无法表达 | — | 缺口 |

### notify

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `notify.export.DiomanNotifyOptions` | 导出类型：公开类型 DiomanNotifyOptions | 无法表达 | — | 缺口 |
| `notify.ctor.DiomanNotifyOptions.new` | 构造方法：构造 DiomanNotifyOptions.new | 无法表达 | — | 缺口 |
| `notify.export.DiomanNotify` | 导出类型：公开类型 DiomanNotify | 无法表达 | — | 缺口 |
| `notify.prop.DiomanNotify.pluginName` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `notify.ctor.DiomanNotify.new` | 构造方法：构造 DiomanNotify.new | 无法表达 | — | 缺口 |
| `notify.opt.notify` | 构造选项：接收通知文本的回调 | 部分 | [G6](#g6) | 已关联 |
| `notify.opt.stringify` | 构造选项：按数据、消息、状态与请求配置生成通知文本 | 部分 | `notify.silent`、`notify.success-no-notify`、`notify.business-error-once`、`notify.cancel-not-notified` | 已关联 |
| `notify.prop.DiomanNotify.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `notify.prop.DiomanNotify.ordering` | 公开属性：插件声明的顺序与依赖约束 | 无法表达 | — | 缺口 |
| `notify.method.DiomanNotify.onRequest` | 公开方法：请求拦截入口 | 部分 | `notify.retry-exhausted-once` | 已关联 |
| `notify.method.DiomanNotify.onResponse` | 公开方法：响应拦截入口 | 部分 | `notify.success-no-notify` | 已关联 |
| `notify.method.DiomanNotify.onError` | 公开方法：错误拦截入口 | 部分 | `notify.http-error-once`、`notify.parse-error-once` | 已关联 |

### core

| API ID | 类别与说明 | schema | 场景 ID | 状态 |
|---|---|---|---|---|
| `core.export.DiomanPlugin` | 导出类型：公开类型 DiomanPlugin | 无法表达 | — | 缺口 |
| `core.ctor.DiomanPlugin.new` | 构造方法：构造 DiomanPlugin.new | 无法表达 | — | 缺口 |
| `core.prop.DiomanPlugin.name` | 公开属性：公开插件协议名或 extra 配置键 | 无法表达 | — | 缺口 |
| `core.prop.DiomanPlugin.ordering` | 公开属性：插件声明的顺序与依赖约束 | 无法表达 | — | 缺口 |
| `core.method.DiomanPlugin.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |
| `core.export.Dioman` | 导出类型：公开类型 Dioman | 无法表达 | — | 缺口 |
| `core.method.Dioman.install` | 公开方法：按标准顺序安装并绑定插件 | 无法表达 | — | 缺口 |
| `core.method.Dioman.installPlugins` | 公开方法：排序、安装任意插件列表并绑定宿主 | 无法表达 | — | 缺口 |
| `core.method.Dioman.sort` | 公开方法：按插件依赖拓扑排序 | 无法表达 | — | 缺口 |
| `core.method.Dioman.validate` | 公开方法：校验既定插件顺序 | 无法表达 | — | 缺口 |
| `core.export.DiomanHandle` | 导出类型：公开类型 DiomanHandle | 无法表达 | — | 缺口 |
| `core.prop.DiomanHandle.plugins` | 公开属性：涉及插件名称列表 | 无法表达 | — | 缺口 |
| `core.method.DiomanHandle.plugin` | 公开方法：按类型查找已安装插件 | 无法表达 | — | 缺口 |
| `core.method.DiomanHandle.remove` | 公开方法：删除缓存条目、mock 路由或插件（以所属类型为准） | 无法表达 | — | 缺口 |
| `core.method.DiomanHandle.insertBefore` | 公开方法：在锚点前插入并校验插件 | 无法表达 | — | 缺口 |
| `core.method.DiomanHandle.insertAfter` | 公开方法：在锚点后插入并校验插件 | 无法表达 | — | 缺口 |
| `core.method.DiomanHandle.prepend` | 公开方法：向插件链头插入并校验 | 无法表达 | — | 缺口 |
| `core.method.DiomanHandle.append` | 公开方法：向插件链尾插入并校验 | 无法表达 | — | 缺口 |
| `core.method.DiomanHandle.diagnoseSharedStateKeys` | 公开方法：诊断重复实例共享 extra 状态键 | 无法表达 | — | 缺口 |
| `core.method.DiomanHandle.dispose` | 公开方法：释放插件资源或解除安装 | 无法表达 | — | 缺口 |
| `core.export.DiomanExtraKeyKind` | 导出枚举：公开枚举 DiomanExtraKeyKind | 无法表达 | — | 缺口 |
| `core.enum.DiomanExtraKeyKind.config` | 枚举值：解析后的构造配置 | 无法表达 | — | 缺口 |
| `core.enum.DiomanExtraKeyKind.protocol` | 枚举值：枚举取值 DiomanExtraKeyKind.protocol | 无法表达 | — | 缺口 |
| `core.enum.DiomanExtraKeyKind.state` | 枚举值：枚举取值 DiomanExtraKeyKind.state | 无法表达 | — | 缺口 |
| `core.export.DiomanExtraKey` | 导出类型：公开类型 DiomanExtraKey | 无法表达 | — | 缺口 |
| `core.ctor.DiomanExtraKey.new` | 构造方法：构造 DiomanExtraKey.new | 无法表达 | — | 缺口 |
| `core.nested.DiomanExtraKey.key` | 嵌套字段：显式请求键 | 无法表达 | — | 缺口 |
| `core.nested.DiomanExtraKey.owner` | 嵌套字段：extra 键所属插件 | 无法表达 | — | 缺口 |
| `core.nested.DiomanExtraKey.kind` | 嵌套字段：extra 键类别 | 无法表达 | — | 缺口 |
| `core.nested.DiomanExtraKey.valueType` | 嵌套字段：extra 键值类型描述 | 无法表达 | — | 缺口 |
| `core.nested.DiomanExtraKey.description` | 嵌套字段：extra 键用途说明 | 无法表达 | — | 缺口 |
| `core.ctor.DiomanExtraKey.config` | 构造方法：解析后的构造配置 | 无法表达 | — | 缺口 |
| `core.parameter.DiomanExtraKey.config(name)` | 构造参数：公开配置 DiomanExtraKey.config(name) | 无法表达 | — | 缺口 |
| `core.parameter.DiomanExtraKey.config(valueType)` | 构造参数：公开配置 DiomanExtraKey.config(valueType) | 无法表达 | — | 缺口 |
| `core.parameter.DiomanExtraKey.config(description)` | 构造参数：公开配置 DiomanExtraKey.config(description) | 无法表达 | — | 缺口 |
| `core.ctor.DiomanExtraKey.state` | 构造方法：构造 DiomanExtraKey.state | 无法表达 | — | 缺口 |
| `core.parameter.DiomanExtraKey.state(key)` | 构造参数：公开配置 DiomanExtraKey.state(key) | 无法表达 | — | 缺口 |
| `core.parameter.DiomanExtraKey.state(owner)` | 构造参数：公开配置 DiomanExtraKey.state(owner) | 无法表达 | — | 缺口 |
| `core.parameter.DiomanExtraKey.state(valueType)` | 构造参数：公开配置 DiomanExtraKey.state(valueType) | 无法表达 | — | 缺口 |
| `core.parameter.DiomanExtraKey.state(description)` | 构造参数：公开配置 DiomanExtraKey.state(description) | 无法表达 | — | 缺口 |
| `core.method.DiomanExtraKey.toString` | 公开方法：undefined | 无法表达 | — | 缺口 |
| `core.export.DiomanSharedStateKey` | 导出类型：公开类型 DiomanSharedStateKey | 无法表达 | — | 缺口 |
| `core.ctor.DiomanSharedStateKey.new` | 构造方法：构造 DiomanSharedStateKey.new | 无法表达 | — | 缺口 |
| `core.nested.DiomanSharedStateKey.key` | 嵌套字段：显式请求键 | 无法表达 | — | 缺口 |
| `core.nested.DiomanSharedStateKey.count` | 嵌套字段：共享相同状态键的实例数 | 无法表达 | — | 缺口 |
| `core.method.DiomanSharedStateKey.toString` | 公开方法：undefined | 无法表达 | — | 缺口 |
| `core.export.DiomanExtraKeys` | 导出类型：公开类型 DiomanExtraKeys | 无法表达 | — | 缺口 |
| `core.prop.DiomanExtraKeys.builtIn` | 公开属性：读取 DiomanExtraKeys.builtIn | 无法表达 | — | 缺口 |
| `core.prop.DiomanExtraKeys.all` | 公开属性：读取 DiomanExtraKeys.all | 无法表达 | — | 缺口 |
| `core.method.DiomanExtraKeys.lookup` | 公开方法：按键查 extra 元数据 | 无法表达 | — | 缺口 |
| `core.method.DiomanExtraKeys.ofKind` | 公开方法：按类别筛选 extra 元数据 | 无法表达 | — | 缺口 |
| `core.method.DiomanExtraKeys.ofOwner` | 公开方法：按插件筛选 extra 元数据 | 无法表达 | — | 缺口 |
| `core.method.DiomanExtraKeys.register` | 公开方法：注册自定义 extra 键；拒绝重复 | 无法表达 | — | 缺口 |
| `core.method.DiomanExtraKeys.unregister` | 公开方法：注销自定义 extra 键 | 无法表达 | — | 缺口 |
| `core.method.DiomanExtraKeys.findSharedState` | 公开方法：检查多实例共享内部状态键 | 无法表达 | — | 缺口 |
| `core.method.DiomanExtraKeys.assertNoSharedState` | 公开方法：断言模式下拒绝共享内部状态键 | 无法表达 | — | 缺口 |
| `core.method.DiomanExtraKeys.toMarkdown` | 公开方法：导出内置 extra 键 Markdown 清单 | 无法表达 | — | 缺口 |
| `core.export.DiomanPluginOrder` | 导出类型：公开类型 DiomanPluginOrder | 无法表达 | — | 缺口 |
| `core.ctor.DiomanPluginOrder.new` | 构造方法：构造 DiomanPluginOrder.new | 无法表达 | — | 缺口 |
| `core.nested.DiomanPluginOrder.before` | 嵌套字段：必须排在指定插件之前 | 无法表达 | — | 缺口 |
| `core.nested.DiomanPluginOrder.after` | 嵌套字段：必须排在指定插件之后 | 无法表达 | — | 缺口 |
| `core.nested.DiomanPluginOrder.requires` | 嵌套字段：依赖插件名称集合 | 无法表达 | — | 缺口 |
| `core.prop.DiomanPluginOrder.none` | 公开属性：读取 DiomanPluginOrder.none | 无法表达 | — | 缺口 |
| `core.export.DiomanPluginOrderIssue` | 导出枚举：公开枚举 DiomanPluginOrderIssue | 无法表达 | — | 缺口 |
| `core.enum.DiomanPluginOrderIssue.duplicateName` | 枚举值：枚举取值 DiomanPluginOrderIssue.duplicateName | 无法表达 | — | 缺口 |
| `core.enum.DiomanPluginOrderIssue.cycle` | 枚举值：枚举取值 DiomanPluginOrderIssue.cycle | 无法表达 | — | 缺口 |
| `core.enum.DiomanPluginOrderIssue.missingDependency` | 枚举值：枚举取值 DiomanPluginOrderIssue.missingDependency | 无法表达 | — | 缺口 |
| `core.enum.DiomanPluginOrderIssue.misordered` | 枚举值：枚举取值 DiomanPluginOrderIssue.misordered | 无法表达 | — | 缺口 |
| `core.export.DiomanPluginOrderException` | 导出类型：公开类型 DiomanPluginOrderException | 无法表达 | — | 缺口 |
| `core.ctor.DiomanPluginOrderException.new` | 构造方法：构造 DiomanPluginOrderException.new | 无法表达 | — | 缺口 |
| `core.nested.DiomanPluginOrderException.issue` | 嵌套字段：顺序违规类别 | 无法表达 | — | 缺口 |
| `core.nested.DiomanPluginOrderException.plugins` | 嵌套字段：涉及插件名称列表 | 无法表达 | — | 缺口 |
| `core.nested.DiomanPluginOrderException.message` | 嵌套字段：异常消息 | 无法表达 | — | 缺口 |
| `core.method.DiomanPluginOrderException.toString` | 公开方法：undefined | 无法表达 | — | 缺口 |

场景集合（完整逐 API 数组也保存在 JSON）：

#### G1

`cache.hit-no-server`、`cache.ttl-expiry`、`cache.http-error-not-cached`、`cache.business-failure-not-cached`、`cache.failure-then-success-cached`、`cache.diff-query-no-mixup`、`cache.diff-body-no-mixup`、`cache.post-not-cached-by-default`、`cache.clear`、`cache.per-request-disable`、`cache.per-request-ttl`、`cache.custom-key-shared`、`cache.user-scope-no-leak`、`cache.user-scope-same-user-hits`、`cache.offline-serves-cache`、`cache.offline-miss-then-recover`、`cache.cancel-not-cached`、`cache.large-response`、`cache.max-entries-evicts`、`cache.timeout-not-cached`、`cache.concurrent-no-share-all-miss`、`combo.cache-share-retry`、`combo.share-failure-not-cached`、`combo.auth-cache-refresh`、`combo.cancel-share-follower`、`combo.cancel-all-share`、`combo.share-retry-loading-notify`、`combo.share-fail-notify`、`combo.loading-cache-hit-delay`、`combo.normalize-business-retry-notify-cache`、`combo.lossy-everything`、`combo.flap-share-cache-loading`、`combo.offline-share-notify`、`combo.full-stack-happy`、`combo.all-seventeen-plugins`、`network.offline-share-loading`、`network.recovery-state-clean`、`offline.cache-hit-still-served`、`key.share-diff-query-not-merged`、`key.query-order-insensitive`、`key.body-diff-not-merged`、`key.method-diff-not-merged`、`key.explicit-string-merges`、`key.filter-before-key`、`repath.key-after-repath`、`repath.key-cache-no-mixup`、`share.start-single-flight`、`share.start-32`、`share.different-keys-independent`、`share.no-cache-after-settle`、`share.per-request-disable`、`share.leader-failure-all-fail`、`share.network-failure-all-fail`、`share.follower-cancel-isolated`、`share.leader-cancel-followers-survive`、`share.all-cancelled-aborts-server`、`share.end-last-wins`、`share.race-first-success`、`share.race-first-fails-second-wins`、`share.none-policy`、`share.reuse-after-failure`

#### G2

`combo.auth-retry-loading-notify-recovers`、`combo.auth-retry-loading-notify-fails`、`combo.auth-concurrent-loading-notify`、`combo.auth-expired-loading-notify`、`combo.retry-cancel-loading-notify`、`combo.share-retry-loading-notify`、`combo.share-fail-notify`、`combo.loading-cache-hit-delay`、`combo.lossy-everything`、`combo.flap-share-cache-loading`、`combo.all-seventeen-plugins`、`loading.single-pair`、`loading.concurrent-single-pair`、`loading.big-parallel-20`、`loading.sequential-pairs`、`loading.failure-closes`、`loading.timeout-closes`、`loading.cancel-closes`、`loading.retry-no-flicker`、`loading.retry-exhausted-closes`、`loading.flap-recovers-single-pair`、`loading.delay-fast-never-shows`、`loading.delay-slow-shows`、`loading.delay-close-merges`、`loading.per-request-off`、`loading.mixed-silent-and-normal`、`loading.parallel-partial-fail`、`loading.network-switch-pairs`、`network.offline-immediate`、`network.flap-retry-recovers`、`network.switch-offline-online`、`network.offline-share-loading`

#### G3

`auth.anon-and-private-mix`、`auth.logout-then-private-fails`、`auth.expired-refresh-replay`、`auth.natural-expiry-refresh`、`auth.two-rounds-refresh-chain`、`auth.concurrent-401-single-refresh-8`、`auth.concurrent-401-single-refresh-32`、`auth.concurrent-two-waves`、`auth.refresh-expired-session`、`auth.refresh-expired-concurrent`、`auth.refresh-fail-times`、`auth.revoke-kicked`、`auth.code-only-401-refresh-once-then-expired`、`auth.code-only-401-recovers`、`auth.code-only-401-no-refresh-token`、`auth.code-only-401-concurrent`、`auth.deny-403`、`auth.public-401-no-refresh`、`auth.public-ignores-expired-token`、`auth.post-replay-body-intact`、`auth.requests-during-refresh-queue`、`auth.logout-inflight`、`auth.no-refresh-user-expired`、`auth.token-none-refresh-ok`、`auth.token-none-refresh-none`、`auth.token-none-refresh-bad`、`auth.token-empty-refresh-ok`、`auth.token-empty-refresh-none`、`auth.token-empty-refresh-bad`、`auth.token-garbage-refresh-ok`、`auth.token-garbage-refresh-none`、`auth.token-garbage-refresh-bad`、`auth.token-bad-signature-refresh-ok`、`auth.token-bad-signature-refresh-none`、`auth.token-bad-signature-refresh-bad`、`auth.token-tampered-payload-refresh-ok`、`auth.token-tampered-payload-refresh-none`、`auth.token-tampered-payload-refresh-bad`、`auth.token-wrong-scheme-refresh-ok`、`auth.token-wrong-scheme-refresh-none`、`auth.token-wrong-scheme-refresh-bad`、`auth.token-garbage-concurrent-refresh-ok`、`auth.token-garbage-concurrent-refresh-bad`、`auth.token-garbage-concurrent-no-refresh`、`auth.token-none-concurrent`、`auth.token-no-refresh-user-garbage`、`auth.token-no-refresh-user-none`、`auth.token-previous-after-logout`、`auth.token-previous-after-revoke`、`auth.token-strict-previous-access`、`auth.token-nonstrict-previous-access`、`auth.token-previous-refresh-reuse`、`auth.strict-rotation-refresh-replay`、`auth.strict-rotation-concurrent`、`auth.strict-rotation-two-rounds`、`auth.network-failure-keeps-session`、`auth.revoke-all-expired`、`auth.revoke-default-scope-is-all`、`auth.revoke-all-concurrent`、`auth.revoke-all-previous-tokens`、`auth.style-switch-refresh`、`auth.style-switch-concurrent`、`auth.style-switch-expired`、`combo.auth-retry-loading-notify-recovers`、`combo.auth-retry-loading-notify-fails`、`combo.auth-concurrent-loading-notify`、`combo.auth-expired-loading-notify`、`combo.auth-cache-refresh`、`combo.full-stack-happy`、`combo.all-seventeen-plugins`、`retry.401-goes-refresh-first`

#### G4

`auth.anon-and-private-mix`、`auth.logout-then-private-fails`、`auth.expired-refresh-replay`、`auth.two-rounds-refresh-chain`、`auth.concurrent-401-single-refresh-8`、`auth.concurrent-401-single-refresh-32`、`auth.concurrent-two-waves`、`auth.refresh-expired-session`、`auth.refresh-expired-concurrent`、`auth.refresh-fail-times`、`auth.relogin-after-expired`、`auth.revoke-kicked`、`auth.code-only-401-refresh-once-then-expired`、`auth.code-only-401-recovers`、`auth.code-only-401-no-refresh-token`、`auth.code-only-401-concurrent`、`auth.deny-403`、`auth.public-401-no-refresh`、`auth.post-replay-body-intact`、`auth.requests-during-refresh-queue`、`auth.slow-refresh-cancel`、`auth.slow-refresh-cancel-all`、`auth.offline-refresh-keeps-session`、`auth.no-refresh-user-expired`、`auth.token-none-refresh-ok`、`auth.token-none-refresh-none`、`auth.token-none-refresh-bad`、`auth.token-empty-refresh-ok`、`auth.token-empty-refresh-none`、`auth.token-empty-refresh-bad`、`auth.token-garbage-refresh-ok`、`auth.token-garbage-refresh-none`、`auth.token-garbage-refresh-bad`、`auth.token-bad-signature-refresh-ok`、`auth.token-bad-signature-refresh-none`、`auth.token-bad-signature-refresh-bad`、`auth.token-tampered-payload-refresh-ok`、`auth.token-tampered-payload-refresh-none`、`auth.token-tampered-payload-refresh-bad`、`auth.token-wrong-scheme-refresh-ok`、`auth.token-wrong-scheme-refresh-none`、`auth.token-wrong-scheme-refresh-bad`、`auth.token-garbage-concurrent-refresh-ok`、`auth.token-garbage-concurrent-refresh-bad`、`auth.token-garbage-concurrent-no-refresh`、`auth.token-none-concurrent`、`auth.token-no-refresh-user-garbage`、`auth.token-no-refresh-user-none`、`auth.token-previous-after-logout`、`auth.token-previous-after-revoke`、`auth.token-strict-previous-access`、`auth.token-nonstrict-previous-access`、`auth.token-previous-refresh-reuse`、`auth.strict-rotation-refresh-replay`、`auth.strict-rotation-concurrent`、`auth.strict-rotation-two-rounds`、`auth.garbage-then-relogin`、`auth.network-failure-keeps-session`、`auth.revoke-all-expired`、`auth.revoke-default-scope-is-all`、`auth.revoke-all-concurrent`、`auth.revoke-all-then-relogin`、`auth.revoke-all-previous-tokens`、`auth.style-switch-refresh`、`auth.style-switch-concurrent`、`auth.style-switch-expired`、`combo.auth-retry-loading-notify-recovers`、`combo.auth-retry-loading-notify-fails`、`combo.auth-expired-loading-notify`、`retry.401-goes-refresh-first`

#### G5

`auth.anon-and-private-mix`、`auth.expired-refresh-replay`、`auth.concurrent-401-single-refresh-8`、`auth.revoke-kicked`、`auth.deny-403`、`auth.post-replay-body-intact`、`auth.token-garbage-refresh-ok`、`auth.token-bad-signature-refresh-ok`、`auth.strict-rotation-refresh-replay`

#### G6

`combo.auth-retry-loading-notify-recovers`、`combo.auth-retry-loading-notify-fails`、`combo.auth-concurrent-loading-notify`、`combo.auth-expired-loading-notify`、`combo.retry-cancel-loading-notify`、`combo.share-retry-loading-notify`、`combo.share-fail-notify`、`combo.normalize-business-retry-notify-cache`、`combo.offline-share-notify`、`combo.full-stack-happy`、`combo.all-seventeen-plugins`、`network.flap-retry-recovers`、`notify.http-error-once`、`notify.silent`、`notify.success-no-notify`、`notify.business-error-once`、`notify.retry-recovered-silent`、`notify.retry-exhausted-once`、`notify.parallel-failures-each`、`notify.cancel-not-notified`、`notify.parse-error-once`、`notify.timeout-once`、`normalize.business-notify-retry-combo`

## 插件两两组合（上三角）

每格为 **共同声明的场景数 / 剔除全插件冒烟后的场景数**；按场景 ID 去重，不乘网络、风格、并发或 repeat。下三角用 —。0 显式保留。

`combo.all-seventeen-plugins` 让全部 136 对的首个数都至少为 1，所以原始矩阵没有 0；剔除它后 **89 对为 0**。第二个数也仅表示共同安装，不能证明双方交互已断言。envs 空规则、log 无事件断言，尤其不能由冒烟升级为 API 覆盖。

| 插件 | envs | timeout | repath | filter | key | normalize | cache | offline | share | mock | cancel | loading | auth | retry | breaker | log | notify |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| envs | — | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 |
| timeout | — | — | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 |
| repath | — | — | — | 1 / 0 | 3 / 2 | 1 / 0 | 2 / 1 | 1 / 0 | 2 / 1 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 |
| filter | — | — | — | — | 2 / 1 | 1 / 0 | 1 / 0 | 1 / 0 | 2 / 1 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 |
| key | — | — | — | — | — | 14 / 13 | 33 / 32 | 2 / 1 | 37 / 36 | 1 / 0 | 8 / 7 | 8 / 7 | 5 / 4 | 8 / 7 | 1 / 0 | 1 / 0 | 6 / 5 |
| normalize | — | — | — | — | — | — | 9 / 8 | 1 / 0 | 9 / 8 | 1 / 0 | 6 / 5 | 11 / 10 | 6 / 5 | 42 / 41 | 11 / 10 | 1 / 0 | 19 / 18 |
| cache | — | — | — | — | — | — | — | 2 / 1 | 7 / 6 | 1 / 0 | 3 / 2 | 5 / 4 | 5 / 4 | 7 / 6 | 1 / 0 | 1 / 0 | 3 / 2 |
| offline | — | — | — | — | — | — | — | — | 1 / 0 | 1 / 0 | 2 / 1 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 | 1 / 0 |
| share | — | — | — | — | — | — | — | — | — | 1 / 0 | 7 / 6 | 7 / 6 | 2 / 1 | 7 / 6 | 1 / 0 | 1 / 0 | 5 / 4 |
| mock | — | — | — | — | — | — | — | — | — | — | 1 / 0 | 1 / 0 | 1 / 0 | 2 / 1 | 1 / 0 | 1 / 0 | 1 / 0 |
| cancel | — | — | — | — | — | — | — | — | — | — | — | 4 / 3 | 3 / 2 | 5 / 4 | 1 / 0 | 1 / 0 | 4 / 3 |
| loading | — | — | — | — | — | — | — | — | — | — | — | — | 6 / 5 | 12 / 11 | 1 / 0 | 1 / 0 | 10 / 9 |
| auth | — | — | — | — | — | — | — | — | — | — | — | — | — | 5 / 4 | 1 / 0 | 1 / 0 | 6 / 5 |
| retry | — | — | — | — | — | — | — | — | — | — | — | — | — | — | 2 / 1 | 1 / 0 | 11 / 10 |
| breaker | — | — | — | — | — | — | — | — | — | — | — | — | — | — | — | 1 / 0 | 1 / 0 |
| log | — | — | — | — | — | — | — | — | — | — | — | — | — | — | — | — | 1 / 0 |
| notify | — | — | — | — | — | — | — | — | — | — | — | — | — | — | — | — | — |

完整两插件场景 ID 列表见 JSON 的 `pairwise`。

## 缺口优先级

先补单插件各 API，再补有交互风险的组合。全部无场景 API（包含无法表达和存疑）已在上表逐项列出；以下按风险分组，未将类型引用算作通过。

| 优先级 | 范围 | 首要缺口与风险 |
|---|---|---|
| P0 | timeout / offline / breaker | timeout 的 probe、分档、send 与请求级覆盖；offline 排队容量、等待、取消、flush/dispose；breaker 半开配额、时钟、桶淘汰、keyBuilder、状态事件及 reset。风险是挂起、资源泄漏、探测风暴、错误桶隔离。 |
| P0 | auth / retry | auth ready、主动刷新 expiresAt/leeway/now、自定义头、请求回调；retry 构造回退、原生 statusCodes、jitter 两种形态、各上限、onRetry、原生 extra 简写。风险是重复提交、无限恢复、令牌复活或退避失控。 |
| P0 | cache / share / mock / cancel | 缓存持久化 read/write/remove/erase、策略、复制、时钟、remove、清理与在途写回；share retry/retries/interval、settle、绑定/解绑；mock 本地 routes/handler/add/remove/reset 和默认网络回退；cancel 原生自建 token 生命周期、构造禁用、导出 cancelAll 返回值。 |
| P0 | envs / log | envs rules/rule/config 的六个字段、首命中和 apply；log 请求/响应/错误、头部、正文、截断、writer 与全部请求级覆盖。共同安装不覆盖这些行为。 |
| P0 | repath / filter / key / normalize / loading / notify | repath removeKey/pattern 与全部请求字段；filter predicate/保留键值/请求并集；key builder/ignores、simple=true、构造开关回退；normalize 自定义字段和回调；loading 请求级回调及延迟、activeCount/dispose；notify 自定义 stringify、兼容空类型和元数据。 |
| P0 | 公共装配 API | core 74 项：动态插入移除、排序依赖错误、重复名/环、extra 注册和诊断、句柄释放；各插件导出类型、枚举、协议名、兼容空操作也需显式契约检查。当前中立 step 无管理操作。 |
| P1 | offline × cancel / share / loading / retry | 队列等待时取消、恢复后只结算一次、loading 配对；maxWait=0 现状掩盖有队列时的交互。即使已有共同场景也缺这条契约。 |
| P1 | timeout × retry / auth / share | 每次尝试超时和逻辑总预算须区分；刷新、退避及共享跟随者不可延长预算或互相取消。宿主总预算不能替代原生分档验证。 |
| P1 | breaker × retry / share / auth | 按真实尝试计数、半开名额、401 不计服务失败；共享跟随者不重复扣配额。已有 retry-halts-on-trip 只覆盖其中一部分。 |
| P1 | cache × auth / key / repath / filter / offline | 用户切换、参数改写后键一致；离线命中、持久化跨用户隔离；clear/remove 与晚到响应不可复活旧缓存。已有用户作用域场景不能代表持久化与清理竞态。 |
| P1 | mock × retry / cache / breaker；log / normalize / notify | mock 回退与重试预算、真实请求键及熔断归属；原始信封可见性、一次通知、日志时机。全插件冒烟不足以判断短路和失败分支。 |

缺口计数：**456 个 API 缺口 = 382 个插件 API + 74 个公共 API**，包含 8 个存疑；**89 个剔除冒烟后为零的插件对**单列，不与 API 缺口相加。已经有关联的 API 仍有取值/边界缺口，未混入这个可复核总数。

## 分层建议

功能层只取 normal；风格仍按场景声明区分，规模 32/64 留在该层验证确定性功能。现有 **301 个场景**有 normal 预期；其余 **27 个**是环境专属，不能把 offline/blackhole 预期直接移到 normal。每场景建议见 JSON 的 `layering`。

环境层候选 **92 个场景**列在下表。仅当网络影响其待验证语义时展开：网络错误分类、断连/慢体、超时、取消、重试恢复、离线探测/排队和网络切换后的状态清理。只复制纯 JSON、token 取值、TTL、正则、过滤、业务码及 barrier 顺序会扩大批量成本，不建议一律展开七档。

| 文件 | 功能层 normal 数 | 环境层候选场景（其余有 normal 的场景仅功能层） |
|---|---:|---|
| auth.json | 69 | `auth.slow-refresh-cancel`、`auth.slow-refresh-cancel-all`、`auth.offline-refresh-keeps-session`、`auth.network-failure-keeps-session` |
| basic.json | 18 |  |
| breaker.json | 11 | `breaker.timeouts-count`、`breaker.network-errors-trip` |
| cache.json | 21 | `cache.offline-serves-cache`、`cache.offline-miss-then-recover`、`cache.cancel-not-cached`、`cache.timeout-not-cached` |
| cancel.json | 11 | `cancel.single`、`cancel.hang-is-canceled-not-timeout`、`cancel.all`、`cancel.all-then-new-works`、`cancel.one-of-many`、`cancel.after-complete-noop`、`cancel.all-with-nothing-inflight`、`cancel.double-cancel`、`cancel.slow-body`、`cancel.before-reach-server`、`cancel.parallel-32`、`cancel.blackhole-cancel-before-timeout` |
| combos.json | 17 | `combo.cancel-share-follower`、`combo.cancel-all-share`、`combo.retry-cancel-loading-notify`、`combo.lossy-everything`、`combo.flap-share-cache-loading`、`combo.offline-share-notify` |
| concurrency.json | 16 |  |
| faults.json | 15 | `faults.hang-timeout`、`faults.close-network`、`faults.half-body`、`faults.slow-body-succeeds`、`faults.close-parallel`、`faults.hang-parallel-timeouts`、`faults.recovery-after-bad`、`faults.post-close-no-double-send` |
| loading.json | 17 | `loading.timeout-closes`、`loading.cancel-closes`、`loading.flap-recovers-single-pair`、`loading.network-switch-pairs` |
| mock.json | 6 |  |
| network.json | 4 | `network.offline-immediate`、`network.offline-all-methods`、`network.blackhole-timeout`、`network.blackhole-parallel-timeouts`、`network.blackhole-cancel-all`、`network.flap-no-retry`、`network.flap-retry-recovers`、`network.flap-parallel-retry`、`network.switch-offline-online`、`network.lossy-parallel-retry`、`network.weak-parallel`、`network.offline-share-loading`、`network.recovery-state-clean`、`network.dns-failure` |
| notify-normalize.json | 19 | `notify.cancel-not-notified`、`notify.timeout-once` |
| offline.json | 2 | `offline.probe-fails-fast`、`offline.online-passes`、`offline.recovers-after-online`、`offline.flap-recovers`、`offline.parallel-all-offline`、`offline.cache-hit-still-served`、`offline.cancel-while-probing`、`offline.then-online-no-stale-state` |
| repath-key-filter.json | 15 |  |
| retry.json | 32 | `retry.cancel-during-wait`、`retry.cancel-all-during-wait`、`retry.network-error-retried`、`retry.post-network-no-retry`、`retry.hang-timeout-retried`、`retry.offline-exhausts`、`retry.blackhole-timeout`、`retry.flap-recovers`、`retry.flap-no-retry-then-ok`、`retry.lossy-converges` |
| share.json | 15 | `share.network-failure-all-fail`、`share.follower-cancel-isolated`、`share.leader-cancel-followers-survive`、`share.all-cancelled-aborts-server` |
| timeout.json | 13 | `timeout.total-hang`、`timeout.receive-slow-body`、`timeout.receive-ok-fast-chunks`、`timeout.total-over-slow-chunks`、`timeout.connect-blackhole`、`timeout.request-override-shorter`、`timeout.request-override-longer`、`timeout.within-budget`、`timeout.recovers-after-timeout`、`timeout.parallel-all-timeout`、`timeout.weak-generous-budget`、`timeout.total-slow-chunks-small`、`timeout.total-budget-slow-body-ok`、`timeout.receive-gap-vs-chunk-bytes` |

网络选择：retry 恢复选 weak/lossy/flap；认证保留会话选 offline/blackhole（DNS 可作同类代表）；取消与超时选 blackhole/weak；offline 探测选 offline/dns/blackhole，恢复选 flap 或显式 network 步骤；breaker 网络失败选 offline/dns。优先保留 JSON 已声明且未 skip 的档位；无对应预期时只是补场景建议，不能算现有覆盖。

weak/lossy/flap 只断言最终结果和资源释放，不复制耗时/次数/顺序精确断言；barrier 按既有矩阵例外处理。离线网络预设与 offline 插件是两个不同维度。上述只是后续分批建议，本次不改场景或执行器。

## 与 axp 的差异

- 按 axp `src/index.ts` 的 bundled plugins：双方共有 auth/cache/retry/share/cancel/loading/mock/normalize/notify/repath/key/filter/envs/log（axp 导出名 axpLogger 对应 log），共 14 个。
- dioman 独有插件：**timeout、breaker、offline**；对应三个专属场景文件，不能作为 axp 缺失 API 来要求实现。
- **axp 独有插件：无**（以本次公开入口快照为准）。公共导出另有差异：axp 提供 Core、AxpResponse/ApiError、具体 TokenManager、pluginLog/Warn/Error 和顶层 removeCache/clearCache；dioman 主要用 Dio/DiomanException、令牌管理接口和 Cache 实例方法，并有顺序/extra 注册诊断 API。这些不等同于插件缺失。

