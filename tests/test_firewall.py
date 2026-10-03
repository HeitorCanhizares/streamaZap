from streamazap import firewall


def test_parse_check_output_detects_block_and_third_party():
    status = firewall.parse_check_output('{"block":2,"allow":1,"thirdParty":["Kaspersky Internet Security","Windows Defender"]}')
    assert status.blocked and status.allowed and not status.ok
    assert status.third_party == ["Kaspersky Internet Security"]
    assert "BLOQUEANDO" in status.describe() and "Kaspersky" in status.describe()


def test_parse_check_output_ok():
    status = firewall.parse_check_output('noise\n{"block":0,"allow":1,"thirdParty":null}')
    assert status.ok and status.describe() == ""


def test_not_supported_outside_frozen_windows():
    assert firewall.program_path() is None
    assert firewall.check().ok
    assert firewall.fix() is False


def test_scripts_quote_program_path():
    script = firewall.CHECK_SCRIPT.format(program=firewall._ps_quote(r"C:\Program Files\It's\StreamaZap.exe"))
    assert r"'C:\Program Files\It''s\StreamaZap.exe'" in script


def test_parse_shielded_profile_and_radmin_category():
    status = firewall.parse_check_output(
        '{"block":0,"allow":1,"shield":"Public","radmin":"Public","thirdParty":[],"rules":[],"profiles":[]}'
    )
    assert status.shielded_profiles == ["Public"] and status.radmin_category == "Public"
    assert status.needs_fix and "Pública" in status.describe()
