"""Finance CSV/OFX/QFX parser with bank preset auto-detection."""
import csv
import re
from datetime import datetime
from pathlib import Path
from typing import Optional


class Transaction:
    """Represents a single financial transaction."""
    __slots__ = ("date", "description", "amount", "category", "is_recurring", "raw")

    def __init__(self, date: datetime, description: str, amount: float, raw: str = ""):
        self.date = date
        self.description = description
        self.amount = amount
        self.category: Optional[str] = None
        self.is_recurring = False
        self.raw = raw

    def to_dict(self) -> dict:
        return {
            "date": self.date.isoformat(),
            "description": self.description,
            "amount": self.amount,
            "category": self.category,
            "is_recurring": self.is_recurring,
        }


# Bank-specific column mappings and date formats
BANK_PRESETS = {
    "nubank": {"date_col": 0, "desc_col": 1, "amount_col": 2, "date_fmt": "%Y-%m-%d"},
    "inter": {"date_col": 0, "desc_col": 2, "amount_col": 3, "date_fmt": "%d/%m/%Y"},
    "itau": {"date_col": 0, "desc_col": 1, "amount_col": 2, "date_fmt": "%d/%m/%Y"},
    "bradesco": {"date_col": 0, "desc_col": 1, "amount_col": 2, "date_fmt": "%d/%m/%Y"},
    "santander": {"date_col": 0, "desc_col": 2, "amount_col": 3, "date_fmt": "%d/%m/%Y"},
    "bb": {"date_col": 0, "desc_col": 1, "amount_col": 2, "date_fmt": "%d/%m/%Y"},
    "caixa": {"date_col": 0, "desc_col": 1, "amount_col": 2, "date_fmt": "%d/%m/%Y"},
    "c6": {"date_col": 0, "desc_col": 1, "amount_col": 2, "date_fmt": "%Y-%m-%d"},
    "picpay": {"date_col": 0, "desc_col": 1, "amount_col": 2, "date_fmt": "%Y-%m-%d"},
    "mercadopago": {"date_col": 0, "desc_col": 1, "amount_col": 2, "date_fmt": "%Y-%m-%d"},
    "generic": {"date_col": 0, "desc_col": 1, "amount_col": 2, "date_fmt": "%Y-%m-%d"},
}


def detect_bank_preset(header_row: list[str]) -> str:
    """Auto-detect bank format from header keywords."""
    header_lower = " ".join(h.lower() for h in header_row)
    if "nubank" in header_lower or "nu bank" in header_lower:
        return "nubank"
    if "inter" in header_lower or "bancointer" in header_lower:
        return "inter"
    if "itaú" in header_lower or "itau" in header_lower:
        return "itau"
    if "bradesco" in header_lower:
        return "bradesco"
    if "santander" in header_lower:
        return "santander"
    if "banco do brasil" in header_lower or "bb" == header_lower.strip():
        return "bb"
    if "caixa" in header_lower or "cef" in header_lower:
        return "caixa"
    if "c6" in header_lower:
        return "c6"
    if "picpay" in header_lower:
        return "picpay"
    if "mercadopago" in header_lower or "mercado pago" in header_lower:
        return "mercadopago"
    return "generic"


def parse_amount(raw: str) -> float:
    """Parse Brazilian or international amount format."""
    cleaned = raw.strip().replace("R$", "").replace("$", "").strip()
    # Brazilian format: 1.234,56 → 1234.56
    if "," in cleaned and "." in cleaned:
        cleaned = cleaned.replace(".", "").replace(",", ".")
    elif "," in cleaned:
        cleaned = cleaned.replace(",", ".")
    return float(cleaned)


def parse_csv(path: str, bank_preset: Optional[str] = None) -> list[Transaction]:
    """Parse a CSV file into transactions."""
    transactions = []
    with open(path, newline="", encoding="utf-8-sig") as f:
        reader = csv.reader(f)
        header = next(reader)
        preset_name = bank_preset or detect_bank_preset(header)
        preset = BANK_PRESETS.get(preset_name, BANK_PRESETS["generic"])

        for row in reader:
            if len(row) <= max(preset["date_col"], preset["desc_col"], preset["amount_col"]):
                continue
            try:
                date_str = row[preset["date_col"]].strip()
                date = datetime.strptime(date_str, preset["date_fmt"])
                desc = row[preset["desc_col"]].strip()
                amount = parse_amount(row[preset["amount_col"]])
                transactions.append(Transaction(date, desc, amount, raw=",".join(row)))
            except (ValueError, IndexError):
                continue
    return transactions


def parse_ofx(path: str) -> list[Transaction]:
    """Parse OFX/QFX file into transactions."""
    transactions = []
    content = Path(path).read_text(encoding="utf-8", errors="replace")
    # Simple regex-based OFX parser for <STMTTRN> blocks
    pattern = re.compile(
        r"<STMTTRN>.*?<DTPOSTED>(\d{8}).*?<TRNAMT>([-\d.]+).*?<MEMO>(.*?)</MEMO>.*?</STMTTRN>",
        re.DOTALL | re.IGNORECASE,
    )
    for match in pattern.finditer(content):
        date_str = match.group(1)[:8]
        date = datetime.strptime(date_str, "%Y%m%d")
        amount = float(match.group(2))
        desc = match.group(3).strip()
        transactions.append(Transaction(date, desc, amount))
    return transactions


def parse_file(path: str, bank_preset: Optional[str] = None) -> list[Transaction]:
    """Auto-detect format and parse."""
    p = Path(path)
    ext = p.suffix.lower()
    if ext in (".ofx", ".qfx"):
        return parse_ofx(path)
    if ext == ".csv":
        return parse_csv(path, bank_preset)
    raise ValueError(f"Unsupported format: {ext}")
