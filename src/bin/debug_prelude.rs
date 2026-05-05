use officium_rs::core::{Date, Locale, OfficeInput, Rubric};
use officium_rs::corpus::BundledCorpus;
use officium_rs::mass::mass_propers;
use officium_rs::precedence;

fn main() {
    let input = OfficeInput {
        date: Date { year: 2026, month: 3, day: 29 },
        rubric: Rubric::Tridentine1570,
        locale: Locale::Latin,
        is_mass_context: true,
    };
    let office = precedence::compute_office(&input, &BundledCorpus);
    let propers = mass_propers(&office, &BundledCorpus);
    if let Some(b) = &propers.evangelium {
        println!("EVANGELIUM ({}):\n{}\n---", b.latin.len(), b.latin);
    }
}
