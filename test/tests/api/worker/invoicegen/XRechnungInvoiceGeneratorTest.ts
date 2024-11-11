import o from "@tutao/otest"
import { createTestEntity } from "../../../TestUtils.js"
import { InvoiceDataGetOutTypeRef, InvoiceDataItemTypeRef } from "../../../../../src/common/api/entities/sys/TypeRefs.js"
import { XRechnungInvoiceGenerator } from "../../../../../src/common/api/worker/invoicegen/XRechnungInvoiceGenerator.js"
import fs from "fs"

o.spec("XRechnungInvoiceGenerator", function () {
	o("xrechnung generation for japanese invoice noVat 2_items", async function () {
		const invoiceData = createTestEntity(InvoiceDataGetOutTypeRef, {
			address: "竜宮 礼奈\n荻町, 411,\n〒501-5627 Shirakawa, Ono-Gun, Gifu, Japan",
			country: "JP",
			subTotal: "20.00",
			grandTotal: "20.00",
			vatType: "0",
			paymentMethod: "0",
			items: [
				createTestEntity(InvoiceDataItemTypeRef, {
					amount: `1`,
					startDate: new Date("09.09.1984"),
					endDate: new Date("09.09.1984"),
					singlePrice: "10.00",
					totalPrice: "10.00",
					itemType: "25",
				}),
				createTestEntity(InvoiceDataItemTypeRef, {
					amount: `1`,
					startDate: new Date("09.09.1984"),
					endDate: new Date("09.09.1984"),
					singlePrice: "10.00",
					totalPrice: "10.00",
					itemType: "25",
				}),
			],
		})
		const gen = new XRechnungInvoiceGenerator(invoiceData, "1978197819801981931", "NiiNii")
		const xml = gen.generate()
		fs.writeFileSync("/tmp/tuta_jp_invoice_noVat_2.xml", xml, { flag: "w" })
	})

	o("xrechnung generation for german paypal addVat 3_items", async function () {
		const invoiceData = createTestEntity(InvoiceDataGetOutTypeRef, {
			address: "Bernd Brot\nNeuschauerberg 56\n91488 Emskirchen",
			country: "DE",
			subTotal: "60.00",
			grandTotal: "71.40",
			vatType: "1",
			vatRate: "19",
			vat: "11.40",
			vatIdNumber: "4444_4444_4444_4444",
			paymentMethod: "3",
			items: [
				createTestEntity(InvoiceDataItemTypeRef, {
					amount: "4",
					startDate: new Date("11.11.1999"),
					endDate: new Date("12.31.2000"),
					singlePrice: "10.00",
					totalPrice: "40.00",
					itemType: "21",
				}),
				createTestEntity(InvoiceDataItemTypeRef, {
					amount: "2",
					startDate: new Date("09.09.1984"),
					endDate: new Date("09.09.1984"),
					singlePrice: "5.00",
					totalPrice: "10.00",
					itemType: "9",
				}),
				createTestEntity(InvoiceDataItemTypeRef, {
					amount: "1",
					startDate: new Date("09.09.1984"),
					endDate: new Date("09.09.1984"),
					singlePrice: "10.00",
					totalPrice: "10.00",
					itemType: "12",
				}),
			],
		})
		const gen = new XRechnungInvoiceGenerator(invoiceData, "1978197819801981931", "NiiNii")
		const xml = gen.generate()
		fs.writeFileSync("/tmp/tuta_de_paypal_addVat_3.xml", xml, { flag: "w" })
	})

	o("xrechnung generation for egypt noVatReverse creditCard addVat 1_items", async function () {
		const invoiceData = createTestEntity(InvoiceDataGetOutTypeRef, {
			address: "Bernd Brot\nNeuschauerberg 56\n91488 Emskirchen",
			country: "DE",
			subTotal: "30.00",
			grandTotal: "35.70",
			vatType: "4",
			vatRate: "19",
			vat: "5.70",
			vatIdNumber: "4444_4444_4444_4444",
			paymentMethod: "2",
			items: [
				createTestEntity(InvoiceDataItemTypeRef, {
					amount: "3",
					startDate: new Date("09.09.1984"),
					endDate: new Date("09.09.1984"),
					singlePrice: "10.00",
					totalPrice: "30.00",
					itemType: "12",
				}),
			],
		})
		const gen = new XRechnungInvoiceGenerator(invoiceData, "1978197819801981931", "NiiNii")
		const xml = gen.generate()
		fs.writeFileSync("/tmp/tuta_de_paypal_addVat_3.xml", xml, { flag: "w" })
	})

	// TODO: How important is postal code parsing? Should extensive regex be used?
	// o("extractPostalCode", function () {
	// 	const line1 = "Berlin, 12107"
	// 	o(extractPostalCode(line1)).equals("12107")
	//
	// 	const line2 = "Neustadt a.d. Aisch 91413"
	// 	o(extractPostalCode(line2)).equals("91413")
	//
	// 	const line3 = "94188 Emskirchen"
	// 	o(extractPostalCode(line3)).equals("91488")
	//
	// 	const line4 = "Holywood BT18 0AA"
	// 	o(extractPostalCode(line4)).equals("BT18 0AA")
	//
	// 	const line5 = "〒501-5627 Shirakawa, Ono-Gun, Gifu, Japan"
	// 	o(extractPostalCode(line5)).equals("〒501-5627")
	// })

	// TODO failing
	// o("extractCityName", function () {
	// 	const line1 = "Berlin, 12107"
	// 	o(extractCityName(line1)).equals("Berlin")
	//
	// 	const line2 = "Neustadt a.d. Aisch 91413"
	// 	o(extractCityName(line2)).equals("Neustadt a.d. Aisch")
	//
	// 	const line3 = "94188 Emskirchen"
	// 	o(extractCityName(line3)).equals("Emskirchen")
	//
	// 	const line4 = "Holywood BT18 0AA"
	// 	o(extractCityName(line4)).equals("Holywood")
	//
	// 	const line5 = "〒501-5627 Shirakawa, Ono-Gun, Gifu, Japan"
	// 	o(extractCityName(line5)).equals("Shirakawa")
	// })
})
