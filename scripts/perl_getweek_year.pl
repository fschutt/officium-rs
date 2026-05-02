#!/usr/bin/env perl
# Phase 2 Perl-side oracle for the Rust↔Perl getweek diff.
# Emits one TSV line per day of YEAR:
#
#   MM-DD<TAB>dow<TAB>week_label
#
# where dow is 0..6 (Sun=0) and week_label is the Perl getweek()
# output (e.g. "Pasc3", "Adv1", "Pent24", "Nat05").
#
# Args:
#   YEAR        (required)
#   missa       (default 1)  — Mass vs Office variant of getweek's tail logic
#   tomorrow    (default 0)  — vesper anticipation flag
#
# See md2json2/src/bin/getweek_check.rs.

use strict;
use warnings;
use FindBin qw($Bin);
use lib "$Bin/../vendor/divinum-officium/web/cgi-bin";
use DivinumOfficium::Date qw(getweek day_of_week leapyear);

my ($year, $missa_arg, $tomorrow_arg) = @ARGV;
defined $year or die "usage: perl_getweek_year.pl YEAR [missa=1] [tomorrow=0]\n";
my $missa    = defined $missa_arg    ? $missa_arg    : 1;
my $tomorrow = defined $tomorrow_arg ? $tomorrow_arg : 0;

my @days_in = (31, leapyear($year) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31);
for my $m (1..12) {
    for my $d (1..$days_in[$m-1]) {
        my $dow = day_of_week($d, $m, $year);
        my $w = getweek($d, $m, $year, $tomorrow, $missa);
        printf "%02d-%02d\t%d\t%s\n", $m, $d, $dow, $w;
    }
}
