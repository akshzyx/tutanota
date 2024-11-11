import o from "@tutao/otest"
import { PdfWriter } from "../../../../../src/common/api/worker/pdf/PdfWriter.js"
import { createTestEntity } from "../../../TestUtils.js"
import { InvoiceDataGetOutTypeRef } from "../../../../../src/common/api/entities/sys/TypeRefs.js"
import { PdfInvoiceGenerator } from "../../../../../src/common/api/worker/invoicegen/PdfInvoiceGenerator.js"
import { object, when } from "testdouble"
import fs from "fs"
import { invoiceItemListMock } from "./invoiceTestUtils.js"

o.spec("PdfInvoiceGenerator", function () {
	let pdfWriter: PdfWriter
	o.beforeEach(function () {
		pdfWriter = new PdfWriter(new TextEncoder(), fetchStub)
	})

	o("pdf generation for japanese invoice addVat 3_items", async function () {
		const invoiceData = createTestEntity(InvoiceDataGetOutTypeRef, {
			address: "竜宮 礼奈\n荻町, 411,\n〒501-5627 Shirakawa, Ono-Gun, Gifu, Japan",
			country: "DE",
			subTotal: "28.00",
			grandTotal: "28.00",
			vatType: "0",
			vatIdNumber: "DE999999999",
			paymentMethod: "0",
			items: invoiceItemListMock(3),
		})

		const gen = new PdfInvoiceGenerator(pdfWriter, invoiceData, "1978197819801981931", "NiiNii")
		const pdf = await gen.generate()
		fs.writeFileSync("/tmp/tuta_jp_invoice_noVat_3.pdf", pdf, { flag: "w" })
	})

	o("pdf rendering 100 entries", async function () {
		const invoiceData = createTestEntity(InvoiceDataGetOutTypeRef, {
			address: "Marcel Davis",
			country: "DE",
			subTotal: "1.00",
			grandTotal: "1.00",
			items: invoiceItemListMock(100),
		})

		const gen = new PdfInvoiceGenerator(pdfWriter, invoiceData, "1978197819801981931", "NiiNii")
		const pdf = await gen.generate()
		fs.writeFileSync("/tmp/tuta_100_entries.pdf", pdf, { flag: "w" })
	})

	o("Entries fit all on a single page but generate a new empty page", async function () {
		const renderInvoice = createTestEntity(InvoiceDataGetOutTypeRef, {
			address: "Altschauerberg 8\n91448 Emskirchen\nDeutschland",
			country: "DE",
			items: invoiceItemListMock(15),
		})
		const gen = new PdfInvoiceGenerator(pdfWriter, renderInvoice, "1978197819801981931", "NiiNii")
		const pdf = await gen.generate()
		fs.writeFileSync("/tmp/tuta_normal_test.pdf", pdf, { flag: "w" })
	})

	o("VatId number is generated", async function () {
		const renderInvoice = createTestEntity(InvoiceDataGetOutTypeRef, {
			address: "BelgianStreet 5\n12345 Zellig\nBelgium",
			country: "BE",
			items: invoiceItemListMock(15),
			vatType: "4",
			vatIdNumber: "1111_2222_3333_4444",
		})
		const gen = new PdfInvoiceGenerator(pdfWriter, renderInvoice, "1978197819801981931", "NiiNii")
		const pdf = await gen.generate()
		// fs.writeFileSync("/tmp/normal_test.pdf", pdf, {flag:"w"})
	})
})

async function fetchStub(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
	if (globalThis.isBrowser) {
		return fetch("./resources/pdf/" + input.toString())
	} else {
		const [fs, path] = await Promise.all([import("node:fs"), import("node:path")])
		const resourceFile = path.normalize(process.cwd() + "/../resources" + input.toString())
		const response: Response = object()
		when(response.arrayBuffer()).thenResolve(fs.readFileSync(resourceFile))
		return response
	}
}
