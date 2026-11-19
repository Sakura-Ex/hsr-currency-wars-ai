"""配置编译各阶段共用的可定位错误。"""

class Unsupported(ValueError):
    """An encountered numeric operation has no validated lowering."""
