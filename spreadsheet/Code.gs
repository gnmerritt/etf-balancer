function onOpen() {
  var ui = SpreadsheetApp.getUi();
  ui.createMenu('Account actions')
    .addItem('Log a deposit', 'contribute')
    .addItem('Rerun balancing', 'remoteBalance')
    .addSeparator()
    .addItem('Record current balance', 'populateBalance')
    .addToUi();
}

function contribute() {
  var ui = SpreadsheetApp.getUi();
  var response = ui.prompt('Log a deposit', 'How much did you deposit?', ui.ButtonSet.OK_CANCEL);

  if (response.getSelectedButton() === ui.Button.OK) {
    var ss = SpreadsheetApp.getActive();
    var amount = response.getResponseText();
    try {
      var deposit = parseInt(amount, 10);
      if (!deposit) return;
      var depositRange = ss.getRangeByName("deposit");
      if (depositRange.getValue() > 0) {
        ui.alert("assign the last deposit first");
        return;
      }
      depositRange.setValue(deposit);
      populateBalance(deposit);
    } catch (e) {
      ui.alert("Couldn't handle that amount " + e);
    }
  }
}

function populateBalanceCron() {
  populateBalance(0); // google puts dumb things into arguments, make sure they don't affect us
}

function populateBalance(cashflow) {
  if (!cashflow) cashflow = 0;
  var ss = SpreadsheetApp.getActive();
  var balanceCell = ss.getRangeByName("currentBalance");
  var balance = balanceCell.getCell(1, 1).getValue();
  if (cashflow > 0) balance = balance + cashflow;

  var ledger = ss.getSheetByName("ledger");
  // loop until we find the first row with an empty date
  for (var insertRow = 1; ledger.getRange(insertRow, 1).getValue(); insertRow++) {
  }

  // insert today's date, cashflow & the balance on the empty row
  ledger.getRange(insertRow, 1).setValue(new Date());
  ledger.getRange(insertRow, 2).setValue(cashflow);
  ledger.getRange(insertRow, 4).setValue(balance);

  // insert account balances too
  var accounts = ss.getRangeByName("account_balances");
  ledger.getRange(insertRow, 7).setValue(accounts.getCell(1, 1).getValue());
  ledger.getRange(insertRow, 8).setValue(accounts.getCell(1, 2).getValue());
  ledger.getRange(insertRow, 9).setValue(accounts.getCell(1, 3).getValue());
}

var URL = 'http://etf.gnmerritt.net/balance';

function remoteBalance() {
  var accounts = buildAccounts();
  const data = buildData();
  var target = {};
  var market = [];

  for (var i = 0; i < data.stocks.length; i++) {
    var s = data.stocks[i];
    target[s.ticker] = s.percent;
    market.push({symbol: s.ticker, price: s.price, div_yield: s.yield });
  }

  var portfolio = {
    accounts: accounts,
    target: target,
    market: market
  };
  var options = {
    'method' : 'post',
    'contentType': 'application/json',
    'payload' : JSON.stringify(portfolio)
  };
  var response = UrlFetchApp.fetch(URL, options);
  var text = response.getContentText();
  var json = JSON.parse(text);

  handleResults(json);
}

function handleResults(results) {
  const ss = SpreadsheetApp.getActive();
  const data = ss.getRangeByName("results");

  for (var i = 1; i <= data.getNumRows(); i++) {
    var symbol = data.getCell(i, SYMBOL).getValue();
    if (!symbol) continue;
    data.getCell(i, TAXED).setValue(balance(results, "taxed", symbol));
    data.getCell(i, ROTH).setValue(balance(results, "roth", symbol));
    data.getCell(i, IRA).setValue(balance(results, "ira", symbol));
  }
}

function balance(results, account, symbol) {
  if (symbol.indexOf("Cash") != -1) {
    return results.cash[account];
  }
  return results.positions[account][symbol] || 0.0;
}

var TAXED = 2;
var ROTH = 3;
var IRA = 4;

function buildAccounts() {
  const ss = SpreadsheetApp.getActive();
  const data = ss.getRangeByName("positions");

  var taxed = account("taxed");
  var roth = account("roth", true);
  var ira = account("ira", true);

  for (var i = 1; i <= data.getNumRows(); i++) {
    var symbol = data.getCell(i, SYMBOL).getValue();
    if (!symbol) { continue; }
    if (symbol.indexOf("cash") != -1) {
      taxed.cash = data.getCell(i, TAXED).getValue();
      roth.cash = data.getCell(i, ROTH).getValue();
      ira.cash = data.getCell(i, IRA).getValue();
    } else {
      taxed.positions[symbol] = data.getCell(i, TAXED).getValue();
      roth.positions[symbol] = data.getCell(i, ROTH).getValue();
      ira.positions[symbol] = data.getCell(i, IRA).getValue();
    }
  }

  return [taxed, roth, ira];
}

function account(name, tax_sheltered) {
  return {
    name: name,
    tax_sheltered: !!tax_sheltered,
    positions: {},
    cash: 0.0
  };
}

var SYMBOL = 1;
var YIELD = 3;
var PERCENT = 4;
var BALANCED = 5;
var ACTUAL = 6;
var DELTA = 7;
var PRICE = 9;

function buildData() {
  const ss = SpreadsheetApp.getActive();
  const data = ss.getRangeByName("data");

  const stocks = [];
  const symbols = [];

  for (var i = 1; i <= data.getNumRows(); i++) {
    var stock = {
      ticker: data.getCell(i, SYMBOL).getValue(),
      yield: data.getCell(i, YIELD).getValue(),
      percent: data.getCell(i, PERCENT).getValue(),
      price: data.getCell(i, PRICE).getValue(),
      current: data.getCell(i, ACTUAL).getValue(),
      balanced: data.getCell(i, BALANCED).getValue(),
      delta: data.getCell(i, DELTA).getValue(),
      needed: 0
    };
    stocks.push(stock);
    symbols.push(stock.ticker);
  }

  return { stocks: stocks, symbols: symbols };
}
