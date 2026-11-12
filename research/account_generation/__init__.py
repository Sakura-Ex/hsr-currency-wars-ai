"""账号生成工具的公开接口。"""

from .accounts.generator import generate_account
from .accounts.models import (
    AccountReference,
    AccountRequest,
    CharacterInvestment,
    GeneratedAccount,
    LightConeReference,
)
from .accounts.references import load_account_reference
from .characters.generator import generate_character
from .characters.models import (
    CharacterConfig,
    CharacterReference,
    CharacterRequest,
    CharacterSpec,
    GeneratedCharacter,
    LightConeInstance,
    LightConeSpec,
    RecommendedTeam,
    RelicInstance,
    TraceConfig,
    TraceReference,
)
from .characters.references import load_character_reference
from .relics.models import (
    CharacterRelicsRequest,
    GeneratedCharacterRelics,
    GeneratedRelic,
    Relic,
    RelicRequest,
    RelicSpec,
)
from .relics.sets import generate_character_relics, maximum_character_relic_score
from .relics.single import GenerationTargetNotReached, generate_relic, score_relic

__all__ = [
    "AccountReference",
    "AccountRequest",
    "CharacterInvestment",
    "GeneratedAccount",
    "LightConeReference",
    "CharacterSpec",
    "RecommendedTeam",
    "generate_account",
    "load_account_reference",
    "CharacterConfig",
    "CharacterReference",
    "CharacterRelicsRequest",
    "CharacterRequest",
    "GeneratedCharacter",
    "GeneratedCharacterRelics",
    "GeneratedRelic",
    "GenerationTargetNotReached",
    "LightConeInstance",
    "LightConeSpec",
    "Relic",
    "RelicInstance",
    "RelicRequest",
    "RelicSpec",
    "TraceConfig",
    "TraceReference",
    "generate_character",
    "generate_character_relics",
    "generate_relic",
    "load_character_reference",
    "maximum_character_relic_score",
    "score_relic",
]
