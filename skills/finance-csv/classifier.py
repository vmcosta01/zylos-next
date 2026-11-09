"""Transaction categorizer with keyword rules and ML fallback."""
import re
from .parser import Transaction

# Keyword → category mapping (Portuguese)
KEYWORD_RULES: list[tuple[list[str], str]] = [
    (["ifood", "uber eats", "rappi", "restaurante", "lanchonete", "padaria", "supermercado", "mercado", "carrefour", "extra", "assai", "atacadao"], "alimentação"),
    (["uber", "99taxi", "cabify", "combustivel", "posto", "estacionamento", "pedagio", "metrô", "metro", "onibus", "ônibus"], "transporte"),
    (["aluguel", "condominio", "condomínio", "iptu", "agua", "água", "luz", "energia", "gas", "gás", "internet", "telefone"], "moradia"),
    (["farmacia", "farmácia", "hospital", "medico", "médico", "plano de saude", "saúde", "unimed", "sulamerica", "consulta"], "saúde"),
    (["curso", "escola", "faculdade", "universidade", "livro", "udemy", "coursera", "mensalidade escolar"], "educação"),
    (["netflix", "spotify", "cinema", "teatro", "show", "jogo", "steam", "playstation", "xbox", "youtube premium"], "lazer"),
    (["amazon", "magalu", "mercadolivre", "mercado livre", "shopee", "aliexpress", "casas bahia", "americanas"], "compras"),
    (["assinatura", "seguro", "contabilidade", "advogado", "cartorio", "cartório"], "serviços"),
    (["pix", "ted", "doc", "transferencia", "transferência"], "transferências"),
]

# Compiled patterns for performance
_COMPILED_RULES = [(re.compile("|".join(re.escape(kw) for kw in keywords), re.IGNORECASE), cat) for keywords, cat in KEYWORD_RULES]


def categorize_by_keywords(description: str) -> str:
    """Categorize using keyword matching."""
    for pattern, category in _COMPILED_RULES:
        if pattern.search(description):
            return category
    return "outros"


def categorize_transactions(transactions: list[Transaction]) -> list[Transaction]:
    """Apply categorization to all transactions."""
    for txn in transactions:
        txn.category = categorize_by_keywords(txn.description)
    return transactions


def detect_recurring(transactions: list[Transaction], threshold_days: int = 35) -> list[Transaction]:
    """Flag transactions that recur monthly (same amount ±5%, within threshold)."""
    by_amount: dict[float, list[Transaction]] = {}
    for txn in transactions:
        key = round(abs(txn.amount), 2)
        by_amount.setdefault(key, []).append(txn)

    for group in by_amount.values():
        if len(group) < 2:
            continue
        group.sort(key=lambda t: t.date)
        for i in range(1, len(group)):
            delta = (group[i].date - group[i - 1].date).days
            if 25 <= delta <= threshold_days:
                group[i].is_recurring = True
                group[i - 1].is_recurring = True
    return transactions
