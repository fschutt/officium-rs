use md2json2::divinum_officium::core::{Date, Locale, OfficeInput, Rubric};
use md2json2::divinum_officium::corpus::BundledCorpus;
use md2json2::divinum_officium::mass::mass_propers;
use md2json2::divinum_officium::precedence;

fn main() {
    let input = OfficeInput {
        date: Date { year: 2026, month: 3, day: 29 },
        rubric: Rubric::Tridentine1570,
        locale: Locale::Latin,
    };
    let office = precedence::compute_office(&input, &BundledCorpus);
    let propers = mass_propers(&office, &BundledCorpus);
    if let Some(b) = &propers.evangelium {
        println!("EVANGELIUM ({}):\n{}\n---", b.latin.len(), b.latin);
    }
}
