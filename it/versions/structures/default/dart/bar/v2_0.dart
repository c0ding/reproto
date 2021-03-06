class Other {
  String name2;

  Other(
    this.name2
  );

  static Other decode(dynamic _dataDyn) {
    if (!(_dataDyn is Map<String, dynamic>)) {
      throw 'expected Map<String, dynamic>, but got: $_dataDyn';
    }

    Map<String, dynamic> _data = _dataDyn;

    var name2_dyn = _data["name2"];
    if (name2_dyn == null) {
      throw "expected value but was null";
    }
    if (!(name2_dyn is String)) {
      throw 'expected String, but was: $name2_dyn';
    }
    final String name2 = name2_dyn;

    return Other(name2);
  }

  Map<String, dynamic> encode() {
    Map<String, dynamic> _data = Map();

    _data["name2"] = this.name2;

    return _data;
  }
}
