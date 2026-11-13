"""五星遗器的主副词条规则。百分比属性使用比例值。"""

# 初始三词条、四词条的候选抽样权重，目标搜索可继续筛选两种起始条件。
INITIAL_SUBSTAT_WEIGHTS = (5, 2)

# 副词条键对应单次低／中／高档数值，已核对并保留数值精度。
SUBSTAT_ROLLS: dict[str, tuple[float, float, float]] = {
    "HP": (33.87004, 38.103798, 42.33755),
    "ATK": (16.935019, 19.051895, 21.168774),
    "DEF": (16.935019, 19.051895, 21.168774),
    "HP_": (0.034560003, 0.038880002, 0.0432),
    "ATK_": (0.034560003, 0.038880002, 0.0432),
    "DEF_": (0.0432, 0.0486, 0.054),
    "SPD": (2.0, 2.3, 2.6),
    "CRIT Rate_": (0.02592, 0.029160001, 0.0324),
    "CRIT DMG_": (0.05184, 0.058320003, 0.0648),
    "Effect Hit Rate_": (0.034560003, 0.038880002, 0.0432),
    "Effect RES_": (0.034560003, 0.038880002, 0.0432),
    "Break Effect_": (0.05184, 0.058320003, 0.0648),
}

# 部位 ID 1～6 依次为头、手、躯干、脚、球、绳，对应各自合法主词条键。
SLOT_MAINSTATS = {
    1: ("HP",),
    2: ("ATK",),
    3: (
        "HP_",
        "ATK_",
        "DEF_",
        "CRIT Rate_",
        "CRIT DMG_",
        "Effect Hit Rate_",
        "Outgoing Healing Boost_",
    ),
    4: ("HP_", "ATK_", "DEF_", "SPD"),
    5: (
        "HP_",
        "ATK_",
        "DEF_",
        "Physical DMG Boost_",
        "Fire DMG Boost_",
        "Ice DMG Boost_",
        "Lightning DMG Boost_",
        "Wind DMG Boost_",
        "Quantum DMG Boost_",
        "Imaginary DMG Boost_",
    ),
    6: ("HP_", "ATK_", "DEF_", "Break Effect_", "Energy Regeneration Rate_"),
}

# 可用套装 ID：外圈 101..134，位面饰品 301..330。
# 各部位随机套装从对应类别采样，名称对照见 docs/game-data/relic-data.md。
SLOT_SET_IDS = {
    slot: tuple(str(i) for i in (range(301, 331) if slot in (5, 6) else range(101, 135)))
    for slot in SLOT_MAINSTATS
}
