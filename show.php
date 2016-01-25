<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="fr" lang="fr">
<head>
<title>Bons comptes</title>
<meta charset="utf-8"/>
<style>
body
{
  background-image: url("images/billets.jpg");
  background-position: center top;
}

p
{
  background-color:#eeeeff;
  opacity:1;
  filter:alpha(opacity=100); /* For IE8 and earlier */
}

table
{
  border-collapse:collapse;
  background-color:#ffffff;
  opacity:0.75;
  filter:alpha(opacity=75); /* For IE8 and earlier */
}

th,td
{
  border: 1px solid black;
}

h1
{
  background-color:#fefeff;
  opacity:1;
  filter:alpha(opacity=100); /* For IE8 and earlier */
}

h2
{
  background-color:#eeeeff;
  opacity:0.75;
  filter:alpha(opacity=75); /* For IE8 and earlier */
}
</style>
</head>
<body>
<?php
  include_once('./functions.php');
  $nodebt = db_connect();

  echo("<form method='get'>\n");

  echo("<h1>Bons comptes</h1>\n");

  echo("<h2>Actions&nbsp;:</h2>\n");
  $user = get_string('user');
  $raw_user = get_raw_string('user');
  $full_user = get_name("$user", $nodebt);

  echo("<p><a href='./insert.php?user=$raw_user'>\n");
  echo("Je viens d'effectuer un paiement, d'autres devraient contribuer\n</a><br />\n");
  echo("<a href='./insert.php?user=$raw_user&contrib_num=0'>\n");
  echo("Je viens de rembourser quelqu'un\n</a><br />\n");
  echo("<a href='./insert.php?receiver=$raw_user'>\n");
  echo("Je viens de recevoir de l'argent, d'autres devraient être remboursés\n</a></p>\n");

  echo("<h2>Affichage&nbsp;:</h2>\n");

  echo("<input type='hidden' name='user' value='$raw_user' />");

  echo("<p>\n<table>\n<tr>\n");
  $i=0;

  $sql_result = $nodebt->query("SELECT fname, uname FROM user WHERE uname <> '$user'");
  if(!$sql_result)
  {
    die($nodebt->error);
  }

  $column_description = array("Date", "Description", "Montant&nbsp;($)", "$full_user");
  $column_id =   array("date", "description", "total", "current");

  $query_select = "";
  $query_join = "";
  $query_where = "";

  while($row = $sql_result->fetch_array(MYSQLI_ASSOC))
  {
    if($i % 4 == 0 && $i != 0)
    {
      echo("</tr>\n<tr>\n");
    }

    $i++;
    echo("<td>\n<input type='checkbox' name='show_$row[uname]'");

    if(!empty($_REQUEST["show_$row[uname]"]))
    {
      echo(" checked ");

      $query_select .= ",\n          ROUND(contrib$i.contribution,2) AS `$row[uname]`";
      $query_join   .= "\n    LEFT JOIN contribution AS contrib$i ON contrib$i.payment = contribution.payment
      AND contrib$i.user = (SELECT id FROM user WHERE uname = '$row[uname]')";

      if($i == 1)
      {
        $query_where .= " AND\n    (\n      ";
      }
      else
      {
        $query_where .= " OR\n      ";
      }


      $query_where .= "payer.uname = '$row[uname]' OR receiver.uname = '$row[uname]' OR contrib$i.contribution > 0.00";

      array_push($column_description, "$row[fname]");
      array_push($column_id, "$row[uname]");
    }

    echo(">$row[fname]</input>\n</td>\n");
  }

  if(!empty($query_where))
  {
    $query_where .= "\n    )";
  }

  array_push($column_description, "Payeur", "Receveur");
  array_push($column_id, "payer", "receiver");

  echo("</tr>\n</table>\n</p>\n");
  $sql_result->close();

  echo("<p><input type='submit' name='update' value='Afficher' /></p>");

  print_table("Transactions", $nodebt, "
    SELECT DATE(date) AS date,
          description,
          ROUND(SUM(contribution.contribution),2) AS total,
          ROUND(current_contrib.contribution,2) AS `current`$query_select,
          payer.fname AS payer,
          receiver.fname AS receiver
    FROM payment
    LEFT JOIN user AS payer ON payment.user = payer.id
    LEFT JOIN user AS receiver ON payment.receiver = receiver.id
    INNER JOIN contribution ON payment.id = contribution.payment
    LEFT JOIN contribution AS current_contrib ON current_contrib.payment = contribution.payment
      AND current_contrib.user = (SELECT id FROM user WHERE uname = '$user')$query_join
    WHERE (payer.uname = '$user' OR receiver.uname = '$user' OR current_contrib.contribution > 0.00)$query_where
    GROUP BY payment.id
    ORDER BY date DESC, payment.id DESC
    ",
    $column_description,
    $column_id
    //array("receiver_fname" => "payer_uname") À mettre entre parenthèse dans la même cellule
  );

  $nodebt->close();

  echo("</form>\n");

?>
</body>
</html>
