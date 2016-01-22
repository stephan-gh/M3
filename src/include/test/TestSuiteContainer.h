/*
 * Copyright (C) 2015, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

#pragma once

#include <m3/Common.h>
#include <m3/util/SList.h>
#include <test/TestSuite.h>

namespace test {

class TestSuiteContainer {
public:
	explicit TestSuiteContainer()
		: _suites() {
	}
	~TestSuiteContainer() {
		for(auto it = _suites.begin(); it != _suites.end(); ) {
			auto old = it++;
			delete &*old;
		}
	}

	void add(TestSuite* suite) {
		_suites.append(suite);
	}
	int run();

private:
	m3::SList<TestSuite> _suites;
};

}
