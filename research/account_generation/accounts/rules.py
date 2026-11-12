"""账号生成的五星份数分类和唯一光锥约定。"""

# 当前参考资料未提供抽卡类别字段，默认按以下 ID 集合分类；加载时可逐条覆盖。
STANDARD_CHARACTER_IDS = frozenset(("1003", "1004", "1101", "1104", "1107", "1209", "1211"))
STANDARD_LIGHT_CONE_IDS = frozenset(("23000", "23002", "23003", "23004", "23005", "23012", "23013"))
# 80xx 常规主角形态及 24xxx 模拟宇宙光锥默认归免费五星类别。
FREE_CHARACTER_PREFIX = "80"
# 22xxx 活动、24xxx 模拟宇宙光锥每种至多一个实例；叠影仍可达 5。
UNIQUE_LIGHT_CONE_PREFIXES = ("22", "24")

# 金数以半金为内部单位；限定一份为 2，常驻一份为 1。
# 免费五星仅在显式纳入统计类别时按一份一金计数，默认预算不含免费类别。
GOLD_HALF_UNITS = {"limited": 2, "standard": 1, "free": 2, "none": 0}
