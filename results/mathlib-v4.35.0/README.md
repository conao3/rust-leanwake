# Mathlib v4.35.0 hotspots

Setup: Mathlib v4.35.0 (56a005855ea17e0b0958f73ec801ae140b0a1679); toolchain: Lean (version 4.35.0, x86_64-unknown-linux-gnu, commit eb6cafb7290e8bd5f2937522d8d804f58f02bafc, Release); leanwake: 6eb83067a3f0cb240c715884711cfcfaa5270882; jobs: 4; CPU: Intel(R) Xeon(R) Processor (nproc: 4); wall time: 22280.20 s; total ElapsedNs: 23153095531036; commands: 416119; files: 8625; FilesWithErrors: 40; exit code: 254.

Recording: 21979.52 s, exit code 254; checkpoint tracing: 300.68 s, exit code 254.

## Files with errors

These files fail only under synchronous elaboration in Lean 4.35.0 (reproducible with `lean -DElab.async=false --setup ...`), so their commands after the failure are missing or errored; Lake builds them cleanly.

Counts are errored commands / recorded commands in errors.json, derived from profile.json, not diagnostic counts.

| Path | First errored command | Errored / recorded commands |
| --- | --- | --- |
| Mathlib/Algebra/Group/End.lean | Equiv.Perm.subtypePerm_inv (line 391) | 4/189 |
| Mathlib/Algebra/Homology/DerivedCategory/Ext/ExtClass.lean | CategoryTheory.ShortComplex.ShortExact.extClass (line 74) | 7/35 |
| Mathlib/Algebra/Lie/BaseChange.lean | LieAlgebra.ExtendScalars.instLieRing (line 98) | 2/45 |
| Mathlib/Algebra/Lie/Weights/RootSystem.lean | LieAlgebra.IsKilling.chainLength (line 64) | 1/53 |
| Mathlib/Algebra/Module/Injective.lean | Module.Baer.ExtensionOfMaxAdjoin.fst (line 238) | 3/75 |
| Mathlib/Algebra/Polynomial/Identities.lean | Polynomial.binomExpansion (line 75) | 1/17 |
| Mathlib/AlgebraicTopology/FundamentalGroupoid/InducedMaps.lean | ContinuousMap.Homotopy.eq_path_of_eq_image (line 156) | 1/46 |
| Mathlib/AlgebraicTopology/SimplicialSet/HomotopyCat.lean | SSet.Truncated.ev12₂ (line 188) | 3/131 |
| Mathlib/Analysis/InnerProductSpace/TensorProduct.lean | TensorProduct.instNormedAddCommGroup (line 133) | 1/218 |
| Mathlib/Analysis/Normed/Lp/PiLp.lean | PiLp.instPseudoEMetricSpace (line 603) | 2/273 |
| Mathlib/Analysis/Normed/Lp/ProdLp.lean | WithLp.instProdPseudoEMetricSpace (line 580) | 2/267 |
| Mathlib/Analysis/Normed/Module/Seminorm/Basic.lean | Seminorm.instConditionallyCompleteLattice (line 570) | 1/275 |
| Mathlib/Analysis/Normed/Operator/Basic.lean | ContinuousLinearMap.toPseudoMetricSpace (line 380) | 1/118 |
| Mathlib/CategoryTheory/Preadditive/CommGrp_.lean | CategoryTheory.Preadditive.commGrpEquivalenceAux (line 62) | 1/14 |
| Mathlib/Combinatorics/Colex.lean | Finset.Colex.le_iff_max'_mem (line 309) | 2/98 |
| Mathlib/Combinatorics/Matroid/Basic.lean | Matroid.diff_subset_ground (line 354) | 2/222 |
| Mathlib/Combinatorics/SetFamily/AhlswedeZhang.lean | Finset.truncatedSup (line 122) | 4/84 |
| Mathlib/Combinatorics/SimpleGraph/Copy.lean | SimpleGraph.definition._@.Mathlib.Combinatorics.SimpleGraph.Copy.3082331781._hygCtx._hyg.14 (line 598) | 10/154 |
| Mathlib/Computability/Primrec/List.lean | Primcodable.list (line 106) | 1/145 |
| Mathlib/Data/Finset/Union.lean | Finset.disjiUnion_filter_eq (line 104) | 2/72 |
| Mathlib/FieldTheory/PerfectClosure.lean | PerfectClosure.instMul (line 136) | 2/67 |
| Mathlib/GroupTheory/DivisibleHull.lean | DivisibleHull.instLE (line 266) | 2/66 |
| Mathlib/LinearAlgebra/CliffordAlgebra/Even.lean | CliffordAlgebra.even.lift.aux (line 191) | 1/37 |
| Mathlib/LinearAlgebra/Dual/BaseChange.lean | IsBaseChange.toDualBaseChange (line 104) | 1/24 |
| Mathlib/LinearAlgebra/LinearPMap.lean | LinearPMap.sup (line 339) | 17/198 |
| Mathlib/LinearAlgebra/RootSystem/Basic.lean | RootPairing.equiv_of_mapsTo (line 68) | 4/32 |
| Mathlib/ModelTheory/Algebra/Ring/FreeCommRing.lean | FirstOrder.Ring.termOfFreeCommRing (line 49) | 2/15 |
| Mathlib/NumberTheory/LegendreSymbol/JacobiSymbol.lean | _private.Mathlib.NumberTheory.LegendreSymbol.JacobiSymbol.0.fastJacobiSym.eq (line 568) | 2/84 |
| Mathlib/NumberTheory/ModularForms/SlashActions.lean | ModularForm.instSlashActionIntGeneralLinearGroupFinOfNatNatRealForallUpperHalfPlaneComplex (line 126) | 1/44 |
| Mathlib/Order/BooleanAlgebra/Basic.lean | GeneralizedBooleanAlgebra.toGeneralizedCoheytingAlgebra (line 122) | 1/135 |
| Mathlib/RingTheory/Coalgebra/TensorProduct.lean | TensorProduct.instCoalgebra (line 122) | 2/62 |
| Mathlib/RingTheory/DividedPowers/Padic.lean | PadicInt.dividedPowers (line 134) | 1/21 |
| Mathlib/RingTheory/DividedPowers/SubDPIdeal.lean | DividedPowers.Quotient.dividedPowers (line 645) | 4/107 |
| Mathlib/RingTheory/PicardGroup.lean | Submodule.tensorEquivMul (line 703) | 1/200 |
| Mathlib/RingTheory/WittVector/Basic.lean | WittVector.ghostMap (line 283) | 1/80 |
| Mathlib/Topology/ExtremallyDisconnected.lean | ExtremallyDisconnected.homeoCompactToT2 (line 242) | 1/32 |
| Mathlib/Topology/MetricSpace/GromovHausdorff.lean | GromovHausdorff.IsometryRel.setoid (line 79) | 1/61 |
| Mathlib/Topology/MetricSpace/GromovHausdorffRealized.lean | GromovHausdorff.candidatesBOfCandidates (line 376) | 5/76 |
| Mathlib/Topology/MetricSpace/Pseudo/Defs.lean | PseudoMetricSpace (line 121) | 26/272 |
| Mathlib/Topology/UnitInterval.lean | Set.Icc.eq_convexComb (line 449) | 1/129 |

## Top 50 declarations

Showing 50 listed declarations. Ranking, seconds and share use the untraced pass. Inside shares use each declaration's traced elapsed time. Source lines are one-based, columns zero-based, and range ends exclusive.

| Rank | Seconds | Share | Path:line | Declaration | Heartbeats | Top Inside entry |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 42.268500 | 0.2% | Mathlib/CategoryTheory/Sites/SheafCohomology/MayerVietoris.lean:123 | CategoryTheory.GrothendieckTopology.MayerVietorisSquare.sequenceIso | 207339561 | Lean.Parser.Tactic.apply 135:19-135:73 x1 1.1% |
| 2 | 24.696637 | 0.1% | Mathlib/AlgebraicGeometry/ProjectiveSpectrum/Functor.lean:179 | AlgebraicGeometry.Proj.awayι_comp_map | 127536796 | Lean.Parser.Tactic.simp 196:2-196:87 x1 1.2% |
| 3 | 23.119181 | 0.1% | Mathlib/CategoryTheory/Abelian/Indization.lean:32 | CategoryTheory.instIsIsoIndCoimageImageComparison | 143516235 | Lean.Parser.Tactic.paren 38:2-39:91 x1 1.2% |
| 4 | 22.813384 | 0.1% | Mathlib/AlgebraicGeometry/Sites/ElladicCohomology.lean:79 | AlgebraicGeometry.Scheme.instAddCommGroupEllAdicCohomology | 115537844 | Kernel x17 97.5% |
| 5 | 18.802749 | 0.1% | Mathlib/AlgebraicGeometry/Modules/Sheaf.lean:309 | AlgebraicGeometry.Scheme.Modules.pseudofunctor | 164714036 | CategoryTheory.cat_disch 322:2-322:33 x3 93.4% |
| 6 | 17.290131 | 0.1% | Mathlib/CategoryTheory/Sites/SheafCohomology/ExactSequences.lean:83 | CategoryTheory.Sheaf.H.longSequenceHom | 97902818 | Lean.Parser.Tactic.simp 91:4-91:81 x5 12.6% |
| 7 | 14.657178 | 0.1% | Mathlib/AlgebraicGeometry/Group/Affine.lean:421 | AlgebraicGeometry.instIsMonHomOverSchemeSpecOfAsOverHomTransPullbackSymmetryOverInferInstanceOverClassPullbackSpecIso' | 114946134 | Lean.Parser.Tactic.simp 433:6-437:74 x1 34.4% |
| 8 | 14.082210 | 0.1% | Mathlib/CategoryTheory/LocallyCartesianClosed/ChosenPullbacksAlong.lean:330 | CategoryTheory.ChosenPullbacksAlong.chosenPullbacksAlongFst | 26388271 | CategoryTheory.cat_disch 337:2-337:16 x2 83.8% |
| 9 | 14.020814 | 0.1% | Mathlib/Algebra/Category/CommHopfAlgCat.lean:203 | commHopfAlgCatEquivCogrpCommAlgCat | 111369701 | CategoryTheory.cat_disch 210:38-210:74 x2 8.6% |
| 10 | 13.180700 | 0.1% | Mathlib/AlgebraicGeometry/EllipticCurve/Projective/Formula.lean:309 | WeierstrassCurve.Projective.negDblY | 113291795 | Elab.definition.value x1 95.0% |
| 11 | 12.209801 | 0.1% | Mathlib/CategoryTheory/Sites/SheafCohomology/Basic.lean:149 | CategoryTheory.Sheaf.functorH | 52705161 | CategoryTheory.cat_disch 153:84-157:0 x2 7.5% |
| 12 | 12.039640 | 0.1% | Mathlib/Algebra/Category/CommBialgCat.lean:211 | commBialgCatEquivComonCommAlgCat | 81074234 | CategoryTheory.cat_disch 218:32-218:78 x2 1.9% |
| 13 | 12.014474 | 0.1% | Mathlib/AlgebraicGeometry/ProjectiveSpectrum/Functor.lean:97 | AlgebraicGeometry.Proj.sheafedSpaceMap | 64955983 | CategoryTheory.cat_disch 103:11-103:12 x1 0.2% |
| 14 | 11.863027 | 0.1% | Mathlib/AlgebraicGeometry/Group/Abelian.lean:38 | AlgebraicGeometry.isCommMonObj_of_isProper_of_isIntegral_tensorObj_of_isAlgClosed | 114620814 | Lean.Parser.Tactic.simp 76:55-76:70 x2 45.3% |
| 15 | 11.273770 | 0.0% | Mathlib/CategoryTheory/Sites/SheafCohomology/ExactSequences.lean:80 | CategoryTheory.Sheaf.H.longSequence_exact | 86633518 | Kernel x1 73.8% |
| 16 | 10.884085 | 0.0% | Mathlib/Topology/ContinuousMap/Periodic.lean:27 | ContinuousMap.periodic_tsum_comp_add_zsmul | 97932829 | Lean.Parser.Tactic.focus 37:4-38:49 x1 94.1% |
| 17 | 10.582508 | 0.0% | Mathlib/CategoryTheory/Sites/SheafCohomology/MayerVietoris.lean:84 | CategoryTheory.GrothendieckTopology.MayerVietorisSquare.biprodAddEquiv_symm_biprodIsoProd_hom_toBiprod_apply | 75493716 | CategoryTheory.cat_disch 90:35-90:44 x1 18.0% |
| 18 | 10.266973 | 0.0% | Mathlib/Analysis/SpecialFunctions/ContinuousFunctionalCalculus/Rpow/RingInverseOrder.lean:35 | CStarAlgebra.convexOn_ringInverse | 103220986 | Lean.Parser.Tactic.grind 99:10-99:59 x1 12.4% |
| 19 | 10.180564 | 0.0% | Mathlib/CategoryTheory/Monoidal/Internal/FunctorCategory.lean:213 | _private.Mathlib.CategoryTheory.Monoidal.Internal.FunctorCategory.0.CategoryTheory.Monoidal.ComonFunctorCategoryEquivalence.unitIso | 10885470 | CategoryTheory.cat_disch 220:2-220:21 x1 79.6% |
| 20 | 10.164412 | 0.0% | Mathlib/NumberTheory/ModularForms/EisensteinSeries/QExpansion.lean:321 | EisensteinSeries.E_qExpansion_coeff | 86349688 | Lean.Parser.Tactic.grind 344:4-344:48 x1 4.0% |
| 21 | 10.125956 | 0.0% | Mathlib/MeasureTheory/Measure/HasOuterApproxClosedProd.lean:75 | Measure.ext_of_lintegral_prod_mul_prod_boundedContinuousFunction | 80485919 | Lean.Parser.Tactic.simp 154:4-155:48 x2 21.5% |
| 22 | 10.012395 | 0.0% | Mathlib/NumberTheory/ModularForms/EisensteinSeries/E2/Summable.lean:106 | EisensteinSeries.hasSum_qExpansion_E2 | 84666867 | Lean.Parser.Tactic.focus 109:2-110:86 x1 93.6% |
| 23 | 9.991599 | 0.0% | Mathlib/AlgebraicGeometry/Morphisms/Flat.lean:335 | AlgebraicGeometry.isIso_pushoutSection_of_iSup_eq | 62681843 | CategoryTheory.cat_disch 376:2-376:3 x1 21.7% |
| 24 | 9.937361 | 0.0% | Mathlib/RingTheory/Etale/QuasiFinite.lean:297 | Algebra.exists_etale_isIdempotentElem_forall_liesOver_eq_aux₂ | 88775696 | Lean.Parser.Tactic.focus 335:4-339:50 x1 4.2% |
| 25 | 9.874216 | 0.0% | Mathlib/LinearAlgebra/RootSystem/Finite/G2.lean:486 | _private.Mathlib.LinearAlgebra.RootSystem.Finite.G2.0.RootPairing.EmbeddedG2.isOrthogonal_short_and_long_aux | 68839054 | Lean.Parser.Tactic.omega 505:2-505:7 x1 93.3% |
| 26 | 9.674014 | 0.0% | Mathlib/Analysis/SpecialFunctions/ContinuousFunctionalCalculus/Rpow/IntegralRepresentation.lean:578 | CFC.concaveOn_cfc_rpowIntegrand₀₁ | 94862341 | Lean.Parser.Tactic.first 591:28-591:41 x1 42.1% |
| 27 | 9.537047 | 0.0% | Mathlib/LinearAlgebra/ExteriorAlgebra/BaseChange.lean:38 | _private.Mathlib.LinearAlgebra.ExteriorAlgebra.BaseChange.0.ExteriorAlgebra.baseChangeι_mul_add_swap | 155098272 | Lean.Parser.Tactic.simpa 46:6-47:43 x1 58.5% |
| 28 | 9.458140 | 0.0% | Mathlib/Algebra/Category/CommAlgCat/Monoidal.lean:54 | CommAlgCat.instMonoidalCategory | 43054462 | CategoryTheory.cat_disch 55:47-65:0 x10 86.3% |
| 29 | 9.449535 | 0.0% | Mathlib/RingTheory/Unramified/LocalStructure.lean:274 | Algebra.IsUnramifiedAt.exists_primesOver_under_adjoin_eq_singleton_and_residueField_bijective | 89503023 | Lean.Parser.Tactic.paren 302:4-302:94 x1 32.8% |
| 30 | 8.879689 | 0.0% | Mathlib/AlgebraicGeometry/Modules/Tilde.lean:253 | AlgebraicGeometry.Scheme.Modules.fromTildeΓ | 62103958 | Lean.Parser.Tactic.paren 283:10-283:37 x1 19.9% |
| 31 | 8.598683 | 0.0% | Mathlib/Tactic/Inclusion/Extension/Interval.lean:332 | Inclusion.Interval.mul_mem | 88894724 | Lean.Parser.Tactic.grind 348:39-348:77 x2 6.3% |
| 32 | 8.268502 | 0.0% | Mathlib/CategoryTheory/Sites/SheafCohomology/MayerVietoris.lean:92 | CategoryTheory.GrothendieckTopology.MayerVietorisSquare.mk₀_f_comp_biprodAddEquiv_symm_biprodIsoProd_hom | 63452657 | CategoryTheory.cat_disch 105:2-105:11 x1 13.3% |
| 33 | 8.190385 | 0.0% | Mathlib/AlgebraicGeometry/EllipticCurve/Projective/Formula.lean:238 | WeierstrassCurve.Projective.dblX | 68182823 | Elab.definition.value x1 93.9% |
| 34 | 8.120630 | 0.0% | Mathlib/AlgebraicGeometry/EllipticCurve/Affine/AddSubMap.lean:242 | WeierstrassCurve.Affine.Point.sym2x_add_sub_eq_addSubMap_sym2x | 39243289 | Lean.Parser.Tactic.grobner 264:37-264:44 x2 70.8% |
| 35 | 7.923169 | 0.0% | Mathlib/AlgebraicGeometry/Normalization.lean:594 | AlgebraicGeometry.Scheme.Hom.instIsIsoNormalizationPullbackOfSmooth | 66828903 | Lean.Parser.Tactic.simp 671:8-671:38 x1 26.9% |
| 36 | 7.838564 | 0.0% | Mathlib/Analysis/InnerProductSpace/OfNorm.lean:144 | InnerProductSpaceable.add_left | 81713097 | Mathlib.Tactic.ModuleNF.moduleNF 158:2-158:23 x1 78.2% |
| 37 | 7.807614 | 0.0% | Mathlib/AlgebraicTopology/SimplicialObject/ChainHomotopy.lean:61 | _private.Mathlib.AlgebraicTopology.SimplicialObject.ChainHomotopy.0.CategoryTheory.SimplicialObject.Homotopy.ToChainHomotopy.comm_succ | 69771617 | Lean.Parser.Tactic.grind 135:45-135:50 x8 31.3% |
| 38 | 7.730166 | 0.0% | Mathlib/NumberTheory/Harmonic/ZetaAsymp.lean:522 | deriv_riemannZeta_zero | 77792341 | Lean.Parser.Tactic.exact 540:6-540:70 x1 62.7% |
| 39 | 7.717568 | 0.0% | Mathlib/AlgebraicGeometry/AffineTransitionLimit.lean:832 | AlgebraicGeometry.exists_appTop_π_eq_of_isLimit | 61093994 | Mathlib.Tactic.convert 854:6-854:58 x1 8.7% |
| 40 | 7.703302 | 0.0% | Mathlib/AlgebraicGeometry/Modules/Tilde.lean:885 | AlgebraicGeometry.tildeEquiv | 47537228 | Lean.Parser.Tactic.dsimp 893:4-893:42 x1 0.4% |
| 41 | 7.332781 | 0.0% | Mathlib/AlgebraicGeometry/Morphisms/Flat.lean:253 | AlgebraicGeometry.mono_pushoutSection_of_iSup_eq | 52524821 | Lean.Parser.Tactic.simp 333:4-333:88 x1 10.9% |
| 42 | 7.244352 | 0.0% | Mathlib/AlgebraicGeometry/EllipticCurve/Affine/AddSubMap.lean:53 | WeierstrassCurve.addSubMapCoeff | 57142961 | Elab.definition.value x1 58.4% |
| 43 | 7.178176 | 0.0% | Mathlib/Algebra/Category/ModuleCat/Sheaf/Quasicoherent.lean:301 | SheafOfModules.QuasicoherentData.pushforward | 63100071 | Lean.Parser.Tactic.exact 330:4-330:74 x1 49.1% |
| 44 | 7.159908 | 0.0% | Mathlib/MeasureTheory/VectorMeasure/WithDensityVec.lean:96 | MeasureTheory.VectorMeasure.variation_withDensity' | 60160922 | Lean.Parser.Tactic.grind 270:56-270:61 x1 9.4% |
| 45 | 7.150156 | 0.0% | Mathlib/Analysis/SpecialFunctions/Elliptic/Weierstrass.lean:1012 | _private.Mathlib.Analysis.SpecialFunctions.Elliptic.Weierstrass.0.PeriodPair.analyticAt_relation_zero | 59968179 | Lean.Parser.Tactic.simp 1036:4-1036:32 x1 27.3% |
| 46 | 7.101657 | 0.0% | Mathlib/AlgebraicGeometry/SpreadingOut.lean:192 | AlgebraicGeometry.spread_out_unique_of_isGermInjective | 84419101 | Lean.Parser.Tactic.obtain 209:2-209:97 x1 0.9% |
| 47 | 7.034591 | 0.0% | Mathlib/AlgebraicTopology/SimplicialSet/HoFunctorMonoidal.lean:355 | SSet.Truncated.HomotopyCategory.BinaryProduct.associativity'Iso_hom_app | 63661840 | Lean.Parser.Tactic.change 358:2-358:26 x1 98.0% |
| 48 | 6.790515 | 0.0% | Mathlib/CategoryTheory/Bicategory/Span/Basic.lean:361 | CategoryTheory.Span.SpanBicat.instBicategory | 57129965 | Lean.Parser.Tactic.grind 374:26-374:59 x1 30.7% |
| 49 | 6.759472 | 0.0% | Mathlib/Algebra/Lie/Cochain.lean:122 | LieModule.Cohomology.d₂₃ | 50981949 | Lean.Parser.Tactic.simp 134:26-134:30 x1 17.1% |
| 50 | 6.595481 | 0.0% | Mathlib/Algebra/Category/ModuleCat/Sheaf/Quasicoherent.lean:428 | SheafOfModules.QuasicoherentData.bind | 51435224 | Lean.Parser.Tactic.exact 443:10-443:71 x1 40.2% |

## Time-consuming tactics

This aggregation covers only the top-50 declarations' traced pass, not all of Mathlib. Only Inside entries with Class Leanwake.tactic contribute; occurrences sum Count, and declarations count distinct (Path, Key) pairs per tactic kind.

| Tactic kind | Seconds | ElapsedNs | Occurrences | Declarations |
| --- | --- | --- | --- | --- |
| CategoryTheory.cat_disch | 53.045561 | 53045561054 | 79 | 12 |
| Lean.Parser.Tactic.simp | 46.901904 | 46901904398 | 257 | 26 |
| Lean.Parser.Tactic.grind | 27.221392 | 27221391513 | 80 | 9 |
| Lean.Parser.Tactic.focus | 21.245806 | 21245806474 | 96 | 21 |
| Lean.Parser.Tactic.omega | 14.583740 | 14583740202 | 3 | 3 |
| Lean.Parser.Tactic.first | 12.953867 | 12953866634 | 35 | 9 |
| Lean.Parser.Tactic.exact | 11.848249 | 11848249062 | 79 | 20 |
| Lean.Parser.Tactic.simpa | 11.652809 | 11652808694 | 29 | 11 |
| Lean.Parser.Tactic.paren | 8.634610 | 8634610156 | 98 | 24 |
| Lean.Parser.Tactic.change | 8.254029 | 8254028988 | 9 | 6 |
| Mathlib.Tactic.ModuleNF.moduleNF | 6.446965 | 6446965308 | 1 | 1 |
| Lean.Parser.Tactic.grobner | 3.726686 | 3726685604 | 2 | 1 |
| Mathlib.Tactic.convert | 2.991734 | 2991734225 | 15 | 10 |
| Lean.Parser.Tactic.refine | 2.936223 | 2936222657 | 105 | 21 |
| Aesop.Frontend.Parser.aesopTactic | 2.596679 | 2596678777 | 10 | 3 |
| Mathlib.Linter.HaveILetI.tacticLetI__ | 2.501785 | 2501784507 | 4 | 1 |
| Lean.Parser.Tactic.dsimp | 2.408007 | 2408007398 | 39 | 13 |
| Mathlib.Meta.FunProp.funPropTacStx | 1.594201 | 1594200607 | 53 | 4 |
| Lean.Parser.Tactic.obtain | 1.526240 | 1526240247 | 19 | 11 |
| Lean.Parser.Tactic.apply | 1.367700 | 1367700464 | 53 | 10 |
| Lean.Elab.Tactic.Ext.ext | 1.327703 | 1327702990 | 34 | 13 |
| Mathlib.Tactic.LinearCombination.linearCombination | 1.025791 | 1025790652 | 1 | 1 |
| Mathlib.Tactic.GCongr.gcongr | 0.630286 | 630286443 | 16 | 4 |
| Lean.Parser.Tactic.congr | 0.568488 | 568487971 | 12 | 5 |
| Mathlib.Tactic.splitIfs | 0.492057 | 492056918 | 3 | 2 |
| Lean.Parser.Tactic.simpaUsingBang | 0.463441 | 463440866 | 2 | 2 |
| Lean.Parser.Tactic.lia | 0.277575 | 277574995 | 1 | 1 |
| Mathlib.Tactic.congrM | 0.240177 | 240177238 | 1 | 1 |
| Mathlib.Tactic.tacticAlgebraize__ | 0.222468 | 222467878 | 3 | 3 |
| Lean.Parser.Tactic.rintro | 0.163458 | 163457826 | 14 | 5 |
| Batteries.Tactic.tacticTrans___ | 0.157032 | 157032152 | 2 | 1 |
| Congr!.congr! | 0.135579 | 135578780 | 4 | 2 |
| Mathlib.Tactic.useSyntax | 0.101276 | 101276137 | 1 | 1 |
| Mathlib.Tactic.applyFun | 0.096248 | 96248394 | 2 | 2 |
| Lean.Parser.Tactic.simpAll | 0.092878 | 92877811 | 2 | 1 |
| Mathlib.Tactic.Positivity.positivity | 0.086027 | 86027301 | 10 | 4 |
| Lean.Parser.Tactic.rcases | 0.083062 | 83062473 | 11 | 2 |
| Mathlib.Linter.HaveILetI.tacticHaveI__ | 0.046747 | 46747467 | 2 | 2 |
| Lean.Parser.Tactic.cases | 0.041869 | 41868951 | 3 | 3 |
| Lean.Parser.Tactic.eqRefl | 0.038445 | 38445102 | 9 | 7 |
| Lean.Parser.Tactic.withAnnotateState | 0.033192 | 33191545 | 59 | 13 |
| Lean.Parser.Tactic.open | 0.031374 | 31374328 | 8 | 4 |
| Lean.Elab.Tactic.finCases | 0.031363 | 31363448 | 1 | 1 |
| Mathlib.Tactic.setTactic | 0.021264 | 21264237 | 2 | 1 |
| Mathlib.Tactic.Choose.choose | 0.020708 | 20708133 | 4 | 2 |
| Mathlib.Tactic.normNum | 0.013577 | 13576897 | 2 | 2 |
| Lean.Parser.Tactic.seq1 | 0.009532 | 9531709 | 1 | 1 |
| Lean.Parser.Tactic.rwa | 0.007139 | 7138568 | 1 | 1 |
| Batteries.Tactic.exacts | 0.007010 | 7009935 | 1 | 1 |
| Lean.Parser.Tactic.intro | 0.006985 | 6984533 | 14 | 8 |
| Lean.Parser.Tactic.constructor | 0.006105 | 6105131 | 10 | 2 |
| Lean.Parser.Tactic.applyRfl | 0.005708 | 5708278 | 1 | 1 |
| Lean.Parser.Tactic.subst | 0.004153 | 4153177 | 3 | 2 |
| Mathlib.Tactic.filterUpwards | 0.003483 | 3482582 | 1 | 1 |
| Mathlib.Tactic.etaExpandStx | 0.001102 | 1101669 | 1 | 1 |
| Lean.Parser.Tactic.unfold | 0.000754 | 754405 | 2 | 2 |
| Lean.Parser.Tactic.withReducible | 0.000570 | 570410 | 3 | 1 |
| Lean.Parser.Tactic.intros | 0.000458 | 457565 | 3 | 1 |
| Lean.Parser.Tactic.symm | 0.000451 | 451325 | 2 | 1 |
| Lean.Parser.Tactic.decide | 0.000322 | 321580 | 1 | 1 |
| Lean.Parser.Tactic.assumption | 0.000178 | 178023 | 2 | 1 |
| Lean.Parser.Tactic.clear | 0.000167 | 166662 | 2 | 2 |
| «tactic#adaptation_note_» | 0.000164 | 163598 | 3 | 1 |
| Mathlib.Tactic.inferOptParam | 0.000131 | 131050 | 3 | 1 |
| Lean.Parser.Tactic.rotateRight | 0.000075 | 74641 | 49 | 15 |
| Batteries.Tactic.tacticPick_goal_ | 0.000057 | 57468 | 1 | 1 |
