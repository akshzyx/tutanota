import { InvoiceDataGetOut, InvoiceDataItem } from "../../entities/sys/TypeRefs.js"
import XRechnungUBLTemplate from "./XRechnungUBLTemplate.js"
import InvoiceTexts from "./InvoiceTexts.js"
import { countryUsesGerman, getInvoiceItemTypeName, InvoiceType, PaymentMethod, VatType } from "./InvoiceUtils.js"

const POSTAL_CODE_REGEX = new RegExp(/.[^\s]*\d.*/)
const CITY_NAME_REGEX = new RegExp(/[^\d]*?(?=,|\s\d|$)/)

const PaymentMethodTypeCodes: Record<PaymentMethod, NumberString> = Object.freeze({
	[PaymentMethod.INVOICE]: "31",
	[PaymentMethod.CREDIT_CARD]: "54",
	[PaymentMethod.SEPA_UNUSED]: "59",
	[PaymentMethod.PAYPAL]: "68",
	[PaymentMethod.ACCOUNT_BALANCE]: "97",
})

const VatTypeCategoryCodes: Record<VatType, string> = Object.freeze({
	[VatType.NO_VAT]: "E",
	[VatType.ADD_VAT]: "S",
	[VatType.VAT_INCLUDED_SHOWN]: "S",
	[VatType.VAT_INCLUDED_HIDDEN]: "S",
	[VatType.NO_VAT_REVERSE_CHARGE]: "AE",
})

/**
 * Object for generating XRechnung invoices.
 * These are electronic invoices conforming to the European standard EN16931 and the German CIUS+Extension XRechnung standard.
 * They are a legal requirement and also improve the billing process for business users.
 * The resulting invoice is an XML file in UBL syntax.
 *
 * This generator is ONLY responsible for processing the data it gets and formatting it in a way that does not change anything about it.
 * If adjustments to the data must be made prior to generation, then these should take place within the RenderInvoice service.
 */
export class XRechnungInvoiceGenerator {
	private readonly languageCode: "de" | "en" = "en"
	private readonly invoiceNumber: string
	private readonly customerId: string
	private readonly buyerMailAddress: string
	private invoice: InvoiceDataGetOut
	private itemIndex: number = 0

	constructor(invoice: InvoiceDataGetOut, invoiceNumber: string, customerId: string, buyerMailAddress: string) {
		this.invoice = invoice
		this.invoiceNumber = invoiceNumber
		this.customerId = customerId
		this.languageCode = countryUsesGerman(this.invoice.country)
		this.buyerMailAddress = buyerMailAddress
	}

	/**
	 * Generate the XRechnung xml file
	 */
	generate(): Uint8Array {
		let stringTemplate = `<?xml version="1.0" encoding="UTF-8"?>` + XRechnungUBLTemplate.Root
		stringTemplate = stringTemplate
			.replace("{invoiceNumber}", this.invoiceNumber) // Unique identifier for the entire invoice
			.replace("{issueDate}", formatDate(this.invoice.date))
			.replace("{buyerId}", this.customerId) // Unique identifier for customer
			.replace("{slotSeller}", XRechnungUBLTemplate.Seller)
			.replace("{slotBuyer}", this.resolveBuyer())
			.replace("{paymentMeansCode}", PaymentMethodTypeCodes[this.invoice.invoiceType as PaymentMethod]) // Payment method
			.replace("{paymentNote}", this.resolvePaymentNote())
			.replace("{slotTotalTax}", this.resolveTotalTax())
			.replace("{slotDocumentTotals}", this.resolveDocumentsTotal())
			.replace("{slotInvoiceLines}", this.resolveInvoiceLines())
		return new TextEncoder().encode(stringTemplate)
	}

	/**
	 * Resolves placeholders concerning the buyer (customer)
	 * buyerMail - Electronic address of the customer
	 * buyerStreetName - despite its name, also includes the street number
	 * buyerCityName - self-explanatory
	 * buyerPostalZone - despite its name, only refers to the postal code, not any associated city
	 * buyerCountryCode - self-explanatory
	 * buyerName - Legal name / company name of the customer -> The first line of the address field
	 * @private
	 */
	private resolveBuyer(): string {
		const addressParts = this.invoice.address.split("\n")
		return XRechnungUBLTemplate.Buyer.replace("{buyerMail}", this.buyerMailAddress)
			.replace("{buyerStreetName}", addressParts[1] ?? "STREET NAME UNKNOWN")
			.replace("{buyerCityName}", extractCityName(addressParts[2] ?? ""))
			.replace("{buyerPostalZone}", extractPostalCode(addressParts[2] ?? ""))
			.replace("{buyerCountryCode}", this.invoice.country)
			.replace("{buyerAddressLine}", this.invoice.address)
			.replace("{slotBuyerVatInfo}", this.resolveBuyerVatInfo())
			.replace("{buyerName}", addressParts[0] ?? "BUYER NAME UNKNOWN")
	}

	/**
	 * Resolves tax info about the buyer (customer). Only resolved if the buyer has a vatIdNumber.
	 * {buyerVatId} - Customer's vatIdNumber
	 * @private
	 */
	private resolveBuyerVatInfo(): string {
		if (this.invoice.vatIdNumber != null) {
			return XRechnungUBLTemplate.BuyerVatInfo.replace("{buyerVatId}", this.invoice.vatIdNumber)
		}
		return ""
	}

	/**
	 * Resolves the payment note, i.e. the instructions for the buyer
	 * These are the same texts below the summary table of a PDF invoice
	 * @private
	 */
	private resolvePaymentNote(): string {
		let paymentNote = ""
		if (this.invoice.invoiceType === InvoiceType.INVOICE) {
			switch (this.invoice.paymentMethod) {
				case PaymentMethod.INVOICE:
					paymentNote += `${InvoiceTexts[this.languageCode].paymentInvoiceDue1} ${InvoiceTexts[this.languageCode].paymentInvoiceDue2} ${
						InvoiceTexts[this.languageCode].paymentInvoiceHolder
					} ${InvoiceTexts[this.languageCode].paymentInvoiceBank} ${InvoiceTexts[this.languageCode].paymentInvoiceIBAN} ${
						InvoiceTexts[this.languageCode].paymentInvoiceBIC
					} ${InvoiceTexts[this.languageCode].paymentInvoiceProvideNumber1} ${this.invoiceNumber} ${
						InvoiceTexts[this.languageCode].paymentInvoiceProvideNumber2
					}`
					break
				case PaymentMethod.CREDIT_CARD:
					paymentNote += `${InvoiceTexts[this.languageCode].paymentCreditCard}`
					break
				case PaymentMethod.PAYPAL:
					paymentNote += `${InvoiceTexts[this.languageCode].paymentPaypal}`
					break
				case PaymentMethod.ACCOUNT_BALANCE:
					paymentNote += `${InvoiceTexts[this.languageCode].paymentAccountBalance}`
					break
			}
			paymentNote += " " + InvoiceTexts[this.languageCode].thankYou
		}
		return paymentNote
	}

	/**
	 * Resolves the total tax slot: summarized information of all applied taxes (vat)
	 * vatType - Standardized VAT category code
	 * vatPercent - Percentage of the vat applied. I.e. 19% -> vatPercent == 19
	 * taxableAmount - Amount that is subject to the tax. Usually this is the entire amount, so the subTotal
	 * vatAmount - The amount of the tax. This is equal to "taxableAmount * vatPercent"
	 * @private
	 */
	private resolveTotalTax(): string {
		return XRechnungUBLTemplate.TaxTotal.replace("{vatType}", VatTypeCategoryCodes[this.invoice.vatType as VatType])
			.replace("{vatPercent}", this.invoice.vatRate)
			.replace("{slotTaxExemptionReason}", this.resolveTaxExemptionReason())
			.replace("{taxableAmount}", this.invoice.subTotal)
			.replaceAll("{vatAmount}", this.invoice.vat)
	}

	/**
	 * Resolves the textual reason why taxes are exempt. Only resolved if the vat type is reverse charge
	 * @private
	 */
	private resolveTaxExemptionReason(): string {
		if (this.invoice.vatType === VatType.NO_VAT_REVERSE_CHARGE) {
			return XRechnungUBLTemplate.TaxExemptionReason
		}
		return ""
	}

	/**
	 * Resolves the document total slot: summarized information of the pricing
	 * sumOfInvoiceLines - The total amount of all invoice items summed up alongside their quantity (amount): subTotal
	 * invoiceExclusiveVat - The total amount of the entire invoice without vat: subTotal
	 * invoiceInclusiveVat - The total amount of the entire invoice with vat: grandTotal
	 * amountDueForPayment - The final amount the buyer is billed with: grandTotal
	 * @private
	 */
	private resolveDocumentsTotal(): string {
		return XRechnungUBLTemplate.DocumentTotals.replace("{sumOfInvoiceLines}", this.invoice.subTotal)
			.replace("{invoiceExclusiveVat}", this.invoice.subTotal)
			.replace("{invoiceInclusiveVat}", this.invoice.grandTotal)
			.replace("{amountDueForPayment}", this.invoice.grandTotal)
	}

	/**
	 * Resolves all invoice items (invoiceLines) by iterating over every invoice item and resolving a list for it
	 * @private
	 */
	private resolveInvoiceLines(): string {
		let invoiceLines = ""
		for (const invoiceItem of this.invoice.items) {
			invoiceLines += this.resolveInvoiceLine(invoiceItem)
		}
		return invoiceLines
	}

	/**
	 * Resolves a singular invoice item (invoiceLine): information about one row in an invoice table
	 * invoiceLineQuantity - The amount (quantity) of the item in the invoice line, so the invoiceItem's amount
	 * invoiceLineTotal - The total price of the invoice line. This is equal to "itemPrice * quantity" == totalPrice
	 * invoiceLineStartDate - self-explanatory
	 * invoiceLineEndDate - self-explanatory
	 * invoiceLineItemName - self-explanatory
	 * invoiceLineItemVatType - Standardized Vat category code for this item. Equal to the vat type of the entire invoice
	 * invoiceLineItemVatPercent - Percentage of vat applied to this item. Equal to the vat percentage of the entire invoice
	 * invoiceLineItemPrice - Price of the singular item: singlePrice
	 * @param invoiceItem
	 * @private
	 */
	private resolveInvoiceLine(invoiceItem: InvoiceDataItem): string {
		this.itemIndex++
		return XRechnungUBLTemplate.InvoiceLine.replace("{invoiceLineId}", this.itemIndex.toString())
			.replace("{invoiceLineQuantity}", invoiceItem.amount)
			.replace("{invoiceLineTotal}", invoiceItem.totalPrice)
			.replace("{invoiceLineStartDate}", formatDate(invoiceItem.startDate))
			.replace("{invoiceLineEndDate}", formatDate(invoiceItem.endDate))
			.replace("{invoiceLineItemName}", getInvoiceItemTypeName(invoiceItem.itemType, this.languageCode))
			.replace("{invoiceLineItemVatType}", VatTypeCategoryCodes[this.invoice.vatType as VatType])
			.replace("{invoiceLineItemVatPercent}", this.invoice.vatRate)
			.replace("{invoiceLineItemPrice}", getInvoiceItemPrice(invoiceItem))
	}

	private isReverseVat(): boolean {
		//return this.invoice.vat === VatType.NO_VAT_REVERSE_CHARGE
		return false
	}
}

/**
 * Formats a date to be of the pattern "yyyy-mm-dd"
 * @param date
 */
function formatDate(date: Date | null): string {
	if (date != null) {
		return date.toISOString().split("T")[0]
	}
	return "No date given."
}

/**
 * Returns the price of an invoice with two decimal places, or "0" if the price is null
 * @param price
 */
function getInvoiceItemPrice(invoiceItem: InvoiceDataItem): string {
	if (invoiceItem.singlePrice != null) {
		return invoiceItem.singlePrice
	}
	return invoiceItem.totalPrice
}

export function extractPostalCode(addressLine: string): string {
	const match = addressLine.match(POSTAL_CODE_REGEX)
	if (match && match[0]) {
		return match[0].trim()
	}
	return "Please refer to the full address line."
}

export function extractCityName(addressLine: string): string {
	const match = addressLine.match(CITY_NAME_REGEX)
	if (match && match[0]) {
		return match[0].trim()
	}
	return "Please refer to the full address line."
}

// TODO: gift cards, invoice types? ProfileID is billing?
// todo: find 0.19 for credits
// todo: test that any combi of pricing stuff gets done proper
