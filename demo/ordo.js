// Static Latin Mass Ordinary — the parts of the Tridentine Mass that
// don't change day to day. Transcribed from the upstream
// `vendor/divinum-officium/web/www/missa/Latin/Ordo/Ordo.txt` (the
// 1962 / Tridentine cursus), folded into a renderer-friendly array.
//
// V1 covers the SPOKEN/SUNG parts only — the silent priestly prayers
// of the Roman Canon (Te igitur … Hanc igitur … Quam oblationem …),
// the offertory secret prayers, and the Last Gospel are abbreviated
// or omitted. Future revisions can flesh those out toggled by a
// "Show silent prayers" mode.
//
// Each entry has a `kind` that the renderer maps to a heading and
// styling:
//   - "rubric"    — instructional italic-red text ("In nomine Patris …")
//   - "spoken"    — dialogue or sung text (Kyrie, Gloria, Sanctus, …)
//   - "proper"    — placeholder for a propers section that the
//                   renderer fills from `compute_mass_json` output
//   - "conditional" — only emitted when a flag is set (`gloria`, `credo`)

export const ORDO = [
  // ── Pre-Mass / Asperges omitted in this V1 ──────────────────────

  // ── Sign of cross + Introit dialogue ────────────────────────────
  { kind: "rubric", body:
    "Signat se signo crucis a fronte ad pectus, et clara voce dicit:" },
  { kind: "spoken", role: "S", body:
    "In nómine Patris, ✠ et Fílii, et Spíritus Sancti. Amen." },

  // ── Asperges / Vidi aquam — TODO: handled by season switch ───────

  { kind: "spoken", role: "S", body: "Introíbo ad altáre Dei." },
  { kind: "spoken", role: "M", body:
    "Ad Deum qui lætíficat juventútem meam." },

  { kind: "rubric", body:
    "Postea alternatim cum Ministris dicit Psalmum 42, omitted on Passiontide " +
    "Sundays and in Masses for the Dead." },
  { kind: "rubric", body:
    "Confíteor … (the priest and ministers exchange the general " +
    "confession; collapsed here for brevity)." },
  { kind: "spoken", role: "S+M", body:
    "Confíteor Deo omnipoténti, … mea culpa, mea culpa, mea máxima culpa. " +
    "Ideo precor beátam Maríam semper Vírginem, … et vos, fratres, oráre " +
    "pro me ad Dóminum, Deum nostrum." },
  { kind: "spoken", role: "M", body:
    "Misereátur tui omnípotens Deus, et, dimíssis peccátis tuis, perdúcat " +
    "te ad vitam ætérnam." },
  { kind: "spoken", role: "S", body: "Amen." },
  { kind: "spoken", role: "S", body:
    "Indulgéntiam, ✠ absolutiónem et remissiónem peccatórum nostrórum " +
    "tríbuat nobis omnípotens et miséricors Dóminus." },
  { kind: "spoken", role: "M", body: "Amen." },

  { kind: "spoken", role: "S", body: "Deus, tu convérsus vivificábis nos." },
  { kind: "spoken", role: "M", body: "Et plebs tua lætábitur in te." },
  { kind: "spoken", role: "S", body: "Osténde nobis, Dómine, misericórdiam tuam." },
  { kind: "spoken", role: "M", body: "Et salutáre tuum da nobis." },
  { kind: "spoken", role: "S", body: "Dómine, exáudi oratiónem meam." },
  { kind: "spoken", role: "M", body: "Et clamor meus ad te véniat." },

  { kind: "rubric", body:
    "Ascendens ad altáre, dicit secreto: Aufer a nobis, quæsumus, Dómine, " +
    "iniquitátes nostras …" },

  // ── Introit (proper) ────────────────────────────────────────────
  { kind: "proper", section: "introitus", header: "Introitus" },

  // ── Kyrie ───────────────────────────────────────────────────────
  { kind: "spoken", role: "S", body: "Kýrie, eléison.", header: "Kyrie" },
  { kind: "spoken", role: "M", body: "Kýrie, eléison." },
  { kind: "spoken", role: "S", body: "Kýrie, eléison." },
  { kind: "spoken", role: "M", body: "Christe, eléison." },
  { kind: "spoken", role: "S", body: "Christe, eléison." },
  { kind: "spoken", role: "M", body: "Christe, eléison." },
  { kind: "spoken", role: "S", body: "Kýrie, eléison." },
  { kind: "spoken", role: "M", body: "Kýrie, eléison." },
  { kind: "spoken", role: "S", body: "Kýrie, eléison." },

  // ── Gloria (conditional on rules.gloria) ────────────────────────
  { kind: "conditional", flag: "gloria", header: "Gloria", entries: [
    { kind: "spoken", role: "S", body:
      "Glória in excélsis Deo. Et in terra pax homínibus bonæ voluntátis. " +
      "Laudámus te. Benedícimus te. Adorámus te. Glorificámus te. Grátias " +
      "ágimus tibi propter magnam glóriam tuam. Dómine Deus, Rex cæléstis, " +
      "Deus Pater omnípotens. Dómine Fili unigénite, Jesu Christe. Dómine " +
      "Deus, Agnus Dei, Fílius Patris. Qui tollis peccáta mundi, miserére " +
      "nobis. Qui tollis peccáta mundi, súscipe deprecatiónem nostram. " +
      "Qui sedes ad déxteram Patris, miserére nobis. Quóniam tu solus " +
      "Sanctus. Tu solus Dóminus. Tu solus Altíssimus, Jesu Christe. Cum " +
      "Sancto Spíritu ✠ in glória Dei Patris. Amen."
    },
  ]},

  // ── Dominus vobiscum + Oratio (proper) ──────────────────────────
  { kind: "spoken", role: "S", body: "Dóminus vobíscum." },
  { kind: "spoken", role: "M", body: "Et cum spíritu tuo." },
  { kind: "spoken", role: "S", body: "Orémus." },
  { kind: "proper", section: "oratio", header: "Oratio" },

  // ── Lectio (proper) ─────────────────────────────────────────────
  { kind: "proper", section: "lectio", header: "Lectio" },
  { kind: "spoken", role: "M", body: "Deo grátias.", suppress_when_empty: "lectio" },

  // ── Graduale + Tractus + Sequentia (propers; one usually empty) ──
  { kind: "proper", section: "graduale", header: "Graduale" },
  { kind: "proper", section: "tractus", header: "Tractus" },
  { kind: "proper", section: "sequentia", header: "Sequentia" },

  // ── Munda cor meum + Iube Domne benedicere ──────────────────────
  { kind: "rubric", body:
    "Sacerdos inclinátus dicit Munda cor meum:" },
  { kind: "spoken", role: "S", body:
    "Munda cor meum ac lábia mea, omnípotens Deus, qui lábia Isaíæ Prophétæ " +
    "cálculo mundásti igníto: ita me tua grata miseratióne dignáre mundáre, " +
    "ut sanctum Evangélium tuum digne váleam nuntiáre. Per Christum, " +
    "Dóminum nostrum. Amen." },
  { kind: "spoken", role: "S", body: "Jube, Dómine, benedícere." },
  { kind: "rubric", body: "Benedíctio:" },
  { kind: "spoken", role: "S", body:
    "Dóminus sit in corde meo et in lábiis meis: ut digne et competénter " +
    "annúntiem Evangélium suum. Amen." },

  // ── Evangelium (proper) ─────────────────────────────────────────
  { kind: "spoken", role: "S", body: "Dóminus vobíscum." },
  { kind: "spoken", role: "M", body: "Et cum spíritu tuo." },
  { kind: "spoken", role: "S", body: "Sequéntia ✠ sancti Evangélii secúndum N." },
  { kind: "spoken", role: "M", body: "Glória tibi, Dómine." },
  { kind: "proper", section: "evangelium", header: "Evangelium" },
  { kind: "spoken", role: "M", body: "Laus tibi, Christe." },
  { kind: "rubric", body:
    "Postea Sacerdos osculátur Evangelium dicens: Per evangélica dicta " +
    "deleántur nostra delícta." },

  // ── Credo (conditional on rules.credo) ──────────────────────────
  { kind: "conditional", flag: "credo", header: "Credo", entries: [
    { kind: "spoken", role: "S", body:
      "Credo in unum Deum, Patrem omnipoténtem, factórem cæli et terræ, " +
      "visibílium ómnium et invisibílium. Et in unum Dóminum Jesum Christum, " +
      "Fílium Dei unigénitum. Et ex Patre natum ante ómnia sǽcula. Deum de " +
      "Deo, Lumen de Lúmine, Deum verum de Deo vero. Génitum, non factum, " +
      "consubstantiálem Patri: per quem ómnia facta sunt. Qui propter nos " +
      "hómines et propter nostram salútem descéndit de cælis. " +
      "(Hic genuflectitur) Et incarnátus est de Spíritu Sancto ex María " +
      "Vírgine: Et homo factus est. Crucifíxus étiam pro nobis: sub Póntio " +
      "Piláto passus, et sepúltus est. Et resurréxit tértia die, secúndum " +
      "Scriptúras. Et ascéndit in cælum: sedet ad déxteram Patris. Et íterum " +
      "ventúrus est cum glória judicáre vivos et mórtuos: cujus regni non " +
      "erit finis. Et in Spíritum Sanctum, Dóminum et vivificántem: qui ex " +
      "Patre Filióque procédit. Qui cum Patre et Fílio simul adorátur et " +
      "conglorificátur: qui locútus est per Prophétas. Et unam sanctam " +
      "cathólicam et apostólicam Ecclésiam. Confíteor unum baptísma in " +
      "remissiónem peccatórum. Et exspécto resurrectiónem mortuórum. ✠ Et " +
      "vitam ventúri sǽculi. Amen."
    },
  ]},

  // ── Offertory: Dominus vobiscum + Oremus + Offertorium proper ───
  { kind: "spoken", role: "S", body: "Dóminus vobíscum." },
  { kind: "spoken", role: "M", body: "Et cum spíritu tuo." },
  { kind: "spoken", role: "S", body: "Orémus." },
  { kind: "proper", section: "offertorium", header: "Offertorium" },

  // ── Silent offertory prayers (collapsed) ────────────────────────
  { kind: "rubric", body:
    "Sacerdos secreto profert: Súscipe, sancte Pater, omnípotens ætérne " +
    "Deus, hanc immaculátam hóstiam … (offerimus … in spíritu humilitátis " +
    "… veni sanctificátor … Lavábo Psalm 25 … Súscipe, sancta Trínitas …)" },

  // ── Orate fratres + Suscipiat ───────────────────────────────────
  { kind: "spoken", role: "S", body:
    "Oráte, fratres: ut meum ac vestrum sacrifícium acceptábile fiat apud " +
    "Deum Patrem omnipoténtem." },
  { kind: "spoken", role: "M", body:
    "Suscípiat Dóminus sacrifícium de mánibus tuis ad laudem et glóriam " +
    "nóminis sui, ad utilitátem quoque nostram, totiúsque Ecclésiæ suæ " +
    "sanctæ." },
  { kind: "spoken", role: "S", body: "Amen." },

  // ── Secreta (proper) — said silently in Low Mass ────────────────
  { kind: "proper", section: "secreta", header: "Secreta" },

  // ── Preface dialog + (proper) Preface ───────────────────────────
  { kind: "spoken", role: "S", body: "Per ómnia sǽcula sæculórum." },
  { kind: "spoken", role: "M", body: "Amen." },
  { kind: "spoken", role: "S", body: "Dóminus vobíscum." },
  { kind: "spoken", role: "M", body: "Et cum spíritu tuo." },
  { kind: "spoken", role: "S", body: "Sursum corda." },
  { kind: "spoken", role: "M", body: "Habémus ad Dóminum." },
  { kind: "spoken", role: "S", body: "Grátias agámus Dómino, Deo nostro." },
  { kind: "spoken", role: "M", body: "Dignum et justum est." },
  { kind: "proper", section: "prefatio", header: "Praefatio",
    fallback_id: "prefatio_communis" },

  // ── Sanctus ─────────────────────────────────────────────────────
  { kind: "spoken", role: "S", header: "Sanctus", body:
    "Sanctus, Sanctus, Sanctus, Dóminus, Deus Sábaoth. Pleni sunt cæli et " +
    "terra glória tua. Hosánna in excélsis. Benedíctus, qui venit in " +
    "nómine Dómini. Hosánna in excélsis." },

  // ── Roman Canon (silent, abbreviated) ───────────────────────────
  { kind: "rubric", header: "Canon Missae", body:
    "Te ígitur, clementíssime Pater … Memento, Dómine … Communicántes … " +
    "Hanc ígitur oblatiónem … Quam oblatiónem … Qui prídie quam paterétur, " +
    "accépit panem … HOC EST ENIM CORPUS MEUM … HIC EST ENIM CALIX SÁNGUINIS " +
    "MEI … (silent priestly prayers, abbreviated for the demo)." },

  // ── Pater Noster + Libera nos ───────────────────────────────────
  { kind: "spoken", role: "S", header: "Pater noster", body:
    "Per ómnia sǽcula sæculórum." },
  { kind: "spoken", role: "M", body: "Amen." },
  { kind: "spoken", role: "S", body: "Orémus. Præcéptis salutáribus móniti, " +
    "et divína institutióne formáti, audémus dícere:" },
  { kind: "spoken", role: "S", body:
    "Pater noster, qui es in cælis: sanctificétur nomen tuum: advéniat " +
    "regnum tuum: fiat volúntas tua, sicut in cælo, et in terra. Panem " +
    "nostrum quotidiánum da nobis hódie: et dimítte nobis débita nostra, " +
    "sicut et nos dimíttimus debitóribus nostris. Et ne nos indúcas in " +
    "tentatiónem:" },
  { kind: "spoken", role: "M", body: "Sed líbera nos a malo." },
  { kind: "spoken", role: "S", body: "Amen." },
  { kind: "rubric", body:
    "Líbera nos, quǽsumus, Dómine, ab ómnibus malis, prætéritis, præséntibus " +
    "et futúris … (silent fraction prayer)." },

  // ── Pax Domini + Agnus Dei ──────────────────────────────────────
  { kind: "spoken", role: "S", body: "Per ómnia sǽcula sæculórum." },
  { kind: "spoken", role: "M", body: "Amen." },
  { kind: "spoken", role: "S", body: "Pax ✠ Dómini sit ✠ semper vobís ✠ cum." },
  { kind: "spoken", role: "M", body: "Et cum spíritu tuo." },

  { kind: "spoken", role: "S", header: "Agnus Dei", body:
    "Agnus Dei, qui tollis peccáta mundi: miserére nobis. " +
    "Agnus Dei, qui tollis peccáta mundi: miserére nobis. " +
    "Agnus Dei, qui tollis peccáta mundi: dona nobis pacem." },

  // ── Pre-communion + Domine non sum dignus ───────────────────────
  { kind: "rubric", body:
    "Sacerdos secreto: Dómine Jesu Christe, qui dixísti Apóstolis tuis: " +
    "Pacem relínquo vobis, pacem meam do vobis … (silent peace prayer)." },
  { kind: "spoken", role: "S", body: "Dómine, non sum dignus, ut intres sub " +
    "tectum meum: sed tantum dic verbo, et sanábitur ánima mea." },
  { kind: "rubric", body:
    "(Repetit ter, percutiens pectus suum.)" },

  // ── Communio (proper) ───────────────────────────────────────────
  { kind: "proper", section: "communio", header: "Communio" },

  // ── Postcommunio: Dominus vobiscum + Oremus + Postcommunio proper ─
  { kind: "spoken", role: "S", body: "Dóminus vobíscum." },
  { kind: "spoken", role: "M", body: "Et cum spíritu tuo." },
  { kind: "spoken", role: "S", body: "Orémus." },
  { kind: "proper", section: "postcommunio", header: "Postcommunio" },

  // ── Ite Missa Est / Benedicamus Domino ──────────────────────────
  { kind: "spoken", role: "S", body: "Dóminus vobíscum." },
  { kind: "spoken", role: "M", body: "Et cum spíritu tuo." },
  // The "Ite Missa Est" form depends on whether Gloria was said:
  //   - Gloria sung  → "Ite, missa est."
  //   - No Gloria    → "Benedicámus Dómino."
  //   - Mass for dead → "Requiéscant in pace."
  { kind: "conditional_branch",
    branches: [
      { when_flag: "gloria", entries: [
        { kind: "spoken", role: "S", body: "Ite, Missa est.", header: "Conclusio" },
        { kind: "spoken", role: "M", body: "Deo grátias." },
      ]},
      { when_default: true, entries: [
        { kind: "spoken", role: "S", body: "Benedicámus Dómino.", header: "Conclusio" },
        { kind: "spoken", role: "M", body: "Deo grátias." },
      ]},
    ]},

  { kind: "rubric", body:
    "Sacerdos inclinátus dicit Pláceat tibi, sancta Trínitas …" },
  { kind: "rubric", body:
    "Postea benedicit pópulum: Benedícat vos omnípotens Deus, Pater, ✠ et " +
    "Fílius, et Spíritus Sanctus." },
  { kind: "spoken", role: "M", body: "Amen." },

  // ── Last Gospel — John 1:1-14 (omitted in this V1; common to all days) ─
  { kind: "rubric", body:
    "Postea legitur Initium sancti Evangélii secúndum Joánnem (Joh 1:1-14), " +
    "abbreviated in this demo." },
];
