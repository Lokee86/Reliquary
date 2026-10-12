
#!/usr/bin/env python3
"""Concrete external-source ownership checks supplement behavioral tests."""
from pathlib import Path
import argparse,re,sys
from check_multiplexing_phase4_coverage import masked,closing
ROOT=Path(__file__).resolve().parents[1]
def check_core(source):
    source=masked(source)
    errors=[]
    forbidden=r"\bEpisode\s*\{|\.(?:append_node|append_node_with_principal|materialize_path_fragments|finalize_import_path_and_queue|put_echo_event)\s*\("
    if re.search(forbidden,source): errors.append("external source may not fabricate/persist native source history")
    return errors
def check_gateway(source):
    source=masked(source);errors=[]
    pattern=r"\bwith_(?:rel|phy|rel_mutation|rel_distribution_admission)\s*\([^;]*?\|[^|]*\|\s*\{"
    for m in re.finditer(pattern,source,re.S):
        start=source.rfind("{",m.start(),m.end())
        body=source[start:closing(source,start)+1]
        if "validate_external_evidence(" in body:
            errors.append("host manifest validation inside owner/distribution admission")
        if "with_phy(" in body and "external_index" in body and "sequencer.gate(" not in body:
            errors.append("external private publication requires source REL sequencer gate")
        if re.search(r"\bsource\s*\.\s*(?:authorize|hydrate|changes_after|snapshot_manifest|source_namespace|contract_version)\s*\(",body):
            errors.append("host call inside owner/distribution admission")
        if re.search(r"\b(?:extractor|endpoint)\s*\.\s*(?:embed|complete_json|extract_external_input)\s*\(",body):
            errors.append("model call inside owner/distribution admission")
    return errors
def main():
    parser=argparse.ArgumentParser();parser.add_argument("--self-test",action="store_true");args=parser.parse_args()
    if args.self_test:
        assert check_core("fn f() { owner.append_node(); }")
        assert check_core("fn f() { let x=Episode { id:fake }; }")
        assert not check_core('fn f() { let s="owner.append_node()"; }')
        assert check_gateway("fn f() { self.with_rel(&t, |r| { source.hydrate(); }); }")
        assert check_gateway("fn f() { self.with_phy(&t, |p| { endpoint.embed(); }); }")
        assert not check_gateway("fn f() { source.hydrate(); self.with_rel(&t, |r| { r.cva.external_search(); }); }")
        assert check_gateway("fn f() { self.with_rel(&t, |r| { validate_external_evidence(source); }); }")
        assert check_gateway("fn f() { self.with_rel_distribution_admission(&t, |_| { let cut=r.cva.external_index; self.with_phy(&p, |p| { publish(); }); }); }")
        assert not check_gateway("fn f() { self.with_rel_distribution_admission(&t, |sequencer| { let _gate=sequencer.gate(t.epoch); let cut=r.cva.external_index; self.with_phy(&p, |p| { publish(); }); }); }")
        assert check_gateway("fn f() { self.with_rel(&t, |r| { source.source_namespace(); }); }")
        assert check_gateway("fn f() { self.with_phy(&t, |p| { source.contract_version(); }); }")
        print("External ownership guard: 11 negative/positive fixtures passed")
    errors=[]
    for path in (ROOT/"src/external_source").glob("*.rs"):
        if path.name=="tests.rs":continue
        errors.extend(f"{path.name}: {e}" for e in check_core(path.read_text(encoding="utf-8")))
    for path in (ROOT/"src/runtime_inference").glob("runtime_host_gateway_external*.rs"):
        if "tests" in path.stem:continue
        errors.extend(f"{path.name}: {e}" for e in check_gateway(path.read_text(encoding="utf-8")))
    if errors:
        print("\n".join(errors),file=sys.stderr);return 1
    print("External ownership guard: production boundaries passed");return 0
if __name__=="__main__":sys.exit(main())
