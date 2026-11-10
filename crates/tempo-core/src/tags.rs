//! Validação de tags e alocações (RN09–RN12).

use crate::error::{CoreError, CoreResult};
use crate::percent::Permille;

pub const MAX_ACTIVE_TAGS: usize = 20;
pub const MAX_NAME_CHARS: usize = 30;

/// Paleta de cores sugeridas (0xRRGGBB), com contraste adequado nos temas claro e escuro.
pub const PALETTE: [u32; 12] = [
    0x3B82F6, // azul
    0x10B981, // verde
    0xF59E0B, // âmbar
    0xEF4444, // vermelho
    0x8B5CF6, // violeta
    0xEC4899, // rosa
    0x14B8A6, // turquesa
    0xF97316, // laranja
    0x6366F1, // índigo
    0x84CC16, // lima
    0x06B6D4, // ciano
    0xA16207, // marrom
];

/// Tags sugeridas na primeira execução (RF14).
pub const SUGGESTED_TAGS: [(&str, u16, u32); 4] =
    [("Trabalho", 500, 0x3B82F6), ("Estudo", 100, 0x8B5CF6), ("Exercício", 50, 0x10B981), ("Lazer", 100, 0xF59E0B)];

/// Normaliza o nome digitado (apara espaços e colapsa espaços internos) e valida o tamanho (RN11).
pub fn normalize_name(raw: &str) -> CoreResult<String> {
    let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(CoreError::TagNameEmpty);
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(CoreError::TagNameTooLong);
    }
    Ok(name)
}

/// Chave de unicidade: minúsculas e sem acentos ("Exercício" == "exercicio").
pub fn name_key(name: &str) -> String {
    name.chars().flat_map(char::to_lowercase).map(fold_accent).collect()
}

fn fold_accent(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        'ý' | 'ÿ' => 'y',
        other => other,
    }
}

/// Verifica se cabe uma nova alocação `requested` dado o total já alocado pelas outras tags (RN09).
pub fn check_allocation(others_total: Permille, requested: Permille) -> CoreResult<()> {
    let total = u32::from(others_total.value()) + u32::from(requested.value());
    if total > 1000 {
        let available = Permille::saturating(1000 - i64::from(others_total.value()));
        return Err(CoreError::AllocationExceeded { available: available.to_string() });
    }
    Ok(())
}

/// Soma das porcentagens.
pub fn total<I: IntoIterator<Item = Permille>>(items: I) -> Permille {
    Permille::saturating(items.into_iter().map(|p| i64::from(p.value())).sum())
}

/// Porcentagem "Não alocado" = 100% − soma (RN09).
pub fn unallocated(total: Permille) -> Permille {
    Permille::saturating(1000 - i64::from(total.value()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nome_normalizado() {
        assert_eq!(normalize_name("  Estudo   de  Rust ").unwrap(), "Estudo de Rust");
        assert_eq!(normalize_name("   "), Err(CoreError::TagNameEmpty));
        assert_eq!(normalize_name(&"a".repeat(30)).unwrap().len(), 30);
        assert_eq!(normalize_name(&"a".repeat(31)), Err(CoreError::TagNameTooLong));
        // 30 caracteres acentuados (mais de 30 bytes) são aceitos
        assert!(normalize_name(&"é".repeat(30)).is_ok());
    }

    #[test]
    fn chave_ignora_caixa_e_acentos() {
        assert_eq!(name_key("Exercício"), name_key("EXERCICIO"));
        assert_eq!(name_key("Ação"), "acao");
        assert_ne!(name_key("Estudo"), name_key("Estudos"));
    }

    #[test]
    fn limite_de_100_por_cento() {
        let p = |v| Permille::new(v).unwrap();
        assert!(check_allocation(p(950), p(50)).is_ok());
        let err = check_allocation(p(950), p(100)).unwrap_err();
        assert_eq!(err, CoreError::AllocationExceeded { available: "5%".into() });
        assert_eq!(err.to_string(), "A soma das porcentagens ultrapassa 100%. Restam apenas 5%.");
    }

    #[test]
    fn total_e_nao_alocado() {
        let t = total([Permille::new(500).unwrap(), Permille::new(100).unwrap(), Permille::new(50).unwrap()]);
        assert_eq!(t.value(), 650);
        assert_eq!(unallocated(t).value(), 350);
        assert_eq!(unallocated(Permille::FULL), Permille::ZERO);
    }

    #[test]
    fn sugestoes_cabem_em_100() {
        let t = total(SUGGESTED_TAGS.iter().map(|&(_, p, _)| Permille::new(p).unwrap()));
        assert!(t.value() <= 1000);
    }
}
